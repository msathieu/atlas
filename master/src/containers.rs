use diesel::prelude::*;
use diesel::{
    PgConnection,
    r2d2::{ConnectionManager, Pool},
};
use k8s_openapi::api::core::v1::{PersistentVolumeClaim, PersistentVolumeClaimSpec, PersistentVolumeClaimVolumeSource, Pod, PodSpec, Volume, VolumeMount, VolumeResourceRequirements};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use kube::api::{AttachParams, DeleteParams, PostParams};
use kube::{Api, Client};
use shared_api::ContainerState;
use shared_server::binary::personalize_binary;
use shared_server::models::{Container, Setting};
use shared_server::notify_server;
use shared_server::schema::{containers, settings};
use std::collections::BTreeMap;
use std::error::Error;
use tokio::io::AsyncWriteExt;

pub async fn process_containers(pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), Box<dyn Error>> {
    let mut db = pool.get()?;
    let removed = diesel::delete(
        containers::table.filter(
            containers::requested_state
                .eq(ContainerState::NonExistent as i32)
                .and(containers::state.eq(ContainerState::NonExistent as i32)),
        ),
    )
    .execute(&mut db)?;
    if removed > 0 {
        notify_server("WEB:REFRESH").await?;
    }
    let containers: Vec<Container> = containers::table.filter(containers::requested_state.ne(containers::state)).get_results(&mut db)?;
    let mut tasks = Vec::new();
    for container in containers {
        let pool = pool.clone();
        tasks.push(tokio::spawn(async {
            if let Err(e) = process_container(container, pool).await {
                println!("Error while processing containers: {}", e);
            }
        }));
    }
    for i in tasks {
        if let Err(e) = i.await {
            println!("Error while processing containers: {}", e);
        }
    }
    Ok(())
}
async fn process_container(container: Container, pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), Box<dyn Error>> {
    let client = Client::try_default().await?;
    let mut db = pool.get()?;
    let container_id = container.id.ok_or("Invalid container")?;
    let container_name = format!("atlas-{}", container_id);
    let pvc_name = format!("atlas-state-{}", container_id);
    let namespace = "atlas";
    if container.state == ContainerState::NonExistent as i32
        && (container.requested_state == ContainerState::Stopped as i32 || container.requested_state == ContainerState::Started as i32 || container.requested_state == ContainerState::Running as i32)
    {
        let storageclass: Setting = settings::table.find("k8s_storageclass").first(&mut db).unwrap();
        let pvc_api = Api::namespaced(client, namespace);
        let mut pvc = PersistentVolumeClaim::default();
        pvc.metadata.name = Some(pvc_name);
        let mut resources = BTreeMap::new();
        resources.insert("storage".to_string(), Quantity("10Gi".to_string()));
        pvc.spec = Some(PersistentVolumeClaimSpec {
            access_modes: Some(vec!["ReadWriteOncePod".to_string()]),
            storage_class_name: Some(storageclass.value),
            resources: Some(VolumeResourceRequirements {
                limits: None,
                requests: Some(resources),
            }),
            ..Default::default()
        });
        pvc_api.create(&PostParams::default(), &pvc).await?;
        diesel::update(containers::table.find(container_id))
            .set(containers::state.eq(ContainerState::Stopped as i32))
            .execute(&mut db)?;
        notify_server("WEB:REFRESH").await?;
        notify_server("MASTER:CONTAINERS").await?;
    } else if container.state == ContainerState::Stopped as i32 && container.requested_state == ContainerState::NonExistent as i32 {
        let pvc_api: Api<PersistentVolumeClaim> = Api::namespaced(client, namespace);
        pvc_api.delete(&pvc_name, &DeleteParams::default()).await?;
        diesel::update(containers::table.find(container_id))
            .set(containers::state.eq(ContainerState::NonExistent as i32))
            .execute(&mut db)?;
        notify_server("WEB:REFRESH").await?;
        notify_server("MASTER:CONTAINERS").await?;
    } else if container.state == ContainerState::Stopped as i32 && (container.requested_state == ContainerState::Started as i32 || container.requested_state == ContainerState::Running as i32) {
        let pod_api = Api::namespaced(client, namespace);
        let mut pod = Pod::default();
        pod.metadata.name = Some(container_name);
        let volume = Volume {
            name: "state".to_string(),
            persistent_volume_claim: Some(PersistentVolumeClaimVolumeSource {
                claim_name: pvc_name,
                read_only: None,
            }),
            ..Default::default()
        };
        let volume_mount = VolumeMount {
            name: "state".to_string(),
            mount_path: "/state".to_string(),
            ..Default::default()
        };
        let k8s_container = k8s_openapi::api::core::v1::Container {
            name: "main".to_string(),
            image: Some(container.image),
            command: Some(vec!["/usr/bin/sleep".to_string(), "infinity".to_string()]),
            volume_mounts: Some(vec![volume_mount]),
            ..Default::default()
        };
        pod.spec = Some(PodSpec {
            share_process_namespace: Some(true),
            volumes: Some(vec![volume]),
            containers: vec![k8s_container],
            ..Default::default()
        });
        pod_api.create(&PostParams::default(), &pod).await?;
        diesel::update(containers::table.find(container_id))
            .set(containers::state.eq(ContainerState::Started as i32))
            .execute(&mut db)?;
        notify_server("WEB:REFRESH").await?;
        notify_server("MASTER:CONTAINERS").await?;
    } else if container.state == ContainerState::Started as i32 && (container.requested_state == ContainerState::Stopped as i32 || container.requested_state == ContainerState::NonExistent as i32) {
        let pod_api: Api<Pod> = Api::namespaced(client, namespace);
        pod_api.delete(&container_name, &DeleteParams::default()).await?;
        diesel::update(containers::table.find(container_id))
            .set(containers::state.eq(ContainerState::Stopped as i32))
            .execute(&mut db)?;
        notify_server("WEB:REFRESH").await?;
        notify_server("MASTER:CONTAINERS").await?;
    } else if container.state == ContainerState::Started as i32 && container.requested_state == ContainerState::Running as i32 {
        let pod_api: Api<Pod> = Api::namespaced(client, namespace);
        let binary = personalize_binary(&mut db, "x86_64", "linux").await?;
        let pod = pod_api.get(&container_name).await?;
        if pod.status.is_none() {
            return Ok(());
        }
        if pod.status.ok_or("Invalid pod")?.phase != Some("Running".to_string()) {
            return Ok(());
        }
        let mut process = pod_api
            .exec(
                &container_name,
                vec![
                    "/usr/bin/sh",
                    "-c",
                    "rm -f /usr/local/bin/atlas-agent && cat > /usr/local/bin/atlas-agent && chmod +x /usr/local/bin/atlas-agent",
                ],
                &AttachParams::default().stdin(true),
            )
            .await?;
        process.stdin().ok_or("Invalid process")?.write_all(&binary).await?;
        process.take_status().ok_or("Invalid process")?.await;
        process.join().await?;
        let mut process = pod_api
            .exec(
                &container_name,
                vec!["/usr/bin/sh", "-c", "/usr/local/bin/atlas-agent supervise > /var/log/atlas.log 2>&1 &"],
                &AttachParams::default(),
            )
            .await?;
        process.take_status().ok_or("Invalid process")?.await;
        process.join().await?;
        diesel::update(containers::table.find(container_id))
            .set(containers::state.eq(ContainerState::Running as i32))
            .execute(&mut db)?;
        notify_server("WEB:REFRESH").await?;
        notify_server("MASTER:CONTAINERS").await?;
    } else if container.state == ContainerState::Running as i32
        && (container.requested_state == ContainerState::Started as i32
            || container.requested_state == ContainerState::Stopped as i32
            || container.requested_state == ContainerState::NonExistent as i32)
    {
        // TODO: Remove the device
        diesel::update(containers::table.find(container_id))
            .set(containers::state.eq(ContainerState::Started as i32))
            .execute(&mut db)?;
        notify_server("WEB:REFRESH").await?;
        notify_server("MASTER:CONTAINERS").await?;
    }
    Ok(())
}
