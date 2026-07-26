use crate::code::PY_DEVELOPMENT_ENVIRONMENT;
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use diesel::prelude::*;
use diesel::{
    PgConnection,
    r2d2::{ConnectionManager, Pool},
};
use k8s_openapi::api::core::v1::{
    EnvVar, LocalObjectReference, PersistentVolumeClaim, PersistentVolumeClaimSpec, PersistentVolumeClaimVolumeSource, Pod, PodSpec, ResourceRequirements, SeccompProfile, SecurityContext, Service,
    ServicePort, ServiceSpec, Volume, VolumeMount, VolumeResourceRequirements,
};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use kube::api::{AttachParams, DeleteParams, PostParams};
use kube::{Api, Client};
use redis::AsyncCommands;
use shared_api::{CommandLanguage, CommandOrigin, ContainerOrigin, ContainerState};
use shared_server::binary::personalize_binary;
use shared_server::models::{Command, Container, DevelopmentEnvironment, Device, Setting};
use shared_server::schema::{commands, containers, development_environments, devices, settings};
use shared_server::{get_valkey_client, notify_server};
use std::collections::BTreeMap;
use std::error::Error;
use std::time::{Duration, SystemTime};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::sleep;
use uuid::Uuid;

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
    let containers: Vec<Container> = containers::table
        .filter(
            containers::requested_state
                .ne(containers::state)
                .or(containers::state.eq(ContainerState::Running as i32))
                .or(containers::state.eq(ContainerState::Started as i32)),
        )
        .get_results(&mut db)?;
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
async fn process_container(mut container: Container, pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), Box<dyn Error>> {
    let client = Client::try_default().await?;
    let mut db = pool.get()?;
    let container_id = container.id.ok_or("Invalid container")?;
    let container_name = format!("atlas-{}", container_id);
    let pvc_name = format!("atlas-state-{}", container_id);
    let nix_pvc_name = format!("atlas-nix-{}", container_id);
    let service_name = format!("atlas-dev-{}", container_id);
    let namespace = "atlas";
    if container.state == ContainerState::Running as i32 || container.state == ContainerState::Started as i32 {
        let pod_api: Api<Pod> = Api::namespaced(client.clone(), namespace);
        let mut request_stop = false;
        match pod_api.get_opt(&container_name).await? {
            Some(pod) => {
                if let Some(status) = pod.status
                    && (status.phase == Some("Succeeded".to_string()) || status.phase == Some("Failed".to_string()))
                {
                    request_stop = true;
                }
            }
            None => {
                request_stop = true;
            }
        }
        if container.origin == ContainerOrigin::DevelopmentEnvironment as i32 {
            let key = format!("DEVELOPMENT_ENVIRONMENT:{}", container_id);
            let exists: usize = get_valkey_client().await?.0.exists(key).await?;
            if exists == 0 {
                request_stop = true;
            }
        }
        if request_stop {
            diesel::update(containers::table.find(container_id))
                .set(containers::requested_state.eq(ContainerState::Stopped as i32))
                .execute(&mut db)?;
            container.requested_state = ContainerState::Stopped as i32;
            notify_server("WEB:REFRESH").await?;
        }
    }
    if container.state == ContainerState::NonExistent as i32
        && (container.requested_state == ContainerState::Stopped as i32 || container.requested_state == ContainerState::Started as i32 || container.requested_state == ContainerState::Running as i32)
    {
        let storageclass: Setting = settings::table.find("k8s_storageclass").first(&mut db).unwrap();
        let pvc_api = Api::namespaced(client.clone(), namespace);
        let mut pvc = PersistentVolumeClaim::default();
        pvc.metadata.name = Some(pvc_name);
        let mut resources = BTreeMap::new();
        resources.insert("storage".to_string(), Quantity("25Gi".to_string()));
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
        if container.origin == ContainerOrigin::DevelopmentEnvironment as i32 {
            let service_api = Api::namespaced(client, namespace);
            let mut service = Service::default();
            service.metadata.name = Some(service_name);
            let mut selector = BTreeMap::new();
            selector.insert("app".to_string(), container_name);
            service.spec = Some(ServiceSpec {
                selector: Some(selector),
                ports: Some(vec![ServicePort { port: 9000, ..Default::default() }]),
                ..Default::default()
            });
            service_api.create(&PostParams::default(), &service).await?;
            pvc.metadata.name = Some(nix_pvc_name);
            pvc_api.create(&PostParams::default(), &pvc).await?;
        }
        notify_server("WEB:REFRESH").await?;
        notify_server("MASTER:CONTAINERS").await?;
    } else if container.state == ContainerState::Stopped as i32 && container.requested_state == ContainerState::NonExistent as i32 {
        let pvc_api: Api<PersistentVolumeClaim> = Api::namespaced(client.clone(), namespace);
        if pvc_api.get_opt(&pvc_name).await?.is_some() {
            pvc_api.delete(&pvc_name, &DeleteParams::default()).await?;
        }
        if container.origin == ContainerOrigin::DevelopmentEnvironment as i32 {
            let service_api: Api<Service> = Api::namespaced(client, namespace);
            if service_api.get_opt(&service_name).await?.is_some() {
                service_api.delete(&service_name, &DeleteParams::default()).await?;
            }
            if pvc_api.get_opt(&nix_pvc_name).await?.is_some() {
                pvc_api.delete(&nix_pvc_name, &DeleteParams::default()).await?;
            }
        }
        diesel::update(containers::table.find(container_id))
            .set(containers::state.eq(ContainerState::NonExistent as i32))
            .execute(&mut db)?;
        notify_server("WEB:REFRESH").await?;
        notify_server("MASTER:CONTAINERS").await?;
    } else if container.state == ContainerState::Stopped as i32 && (container.requested_state == ContainerState::Started as i32 || container.requested_state == ContainerState::Running as i32) {
        let pod_api = Api::namespaced(client, namespace);
        let mut pod = Pod::default();
        pod.metadata.name = Some(container_name.clone());
        let mut labels = BTreeMap::new();
        labels.insert("app".to_string(), container_name);
        if container.origin == ContainerOrigin::DevelopmentEnvironment as i32 {
            labels.insert("atlas-type".to_string(), "development-environment".to_string());
        }
        pod.metadata.labels = Some(labels);
        let volume = Volume {
            name: "state".to_string(),
            persistent_volume_claim: Some(PersistentVolumeClaimVolumeSource {
                claim_name: pvc_name,
                read_only: None,
            }),
            ..Default::default()
        };
        let mut volumes = vec![volume];
        if container.origin == ContainerOrigin::DevelopmentEnvironment as i32 {
            let nix_volume = Volume {
                name: "nix".to_string(),
                persistent_volume_claim: Some(PersistentVolumeClaimVolumeSource {
                    claim_name: nix_pvc_name,
                    read_only: None,
                }),
                ..Default::default()
            };
            volumes.push(nix_volume);
        }
        let volume_mount = VolumeMount {
            name: "state".to_string(),
            mount_path: "/state".to_string(),
            ..Default::default()
        };
        let mut volume_mounts = vec![volume_mount];
        if container.origin == ContainerOrigin::DevelopmentEnvironment as i32 {
            let nix_volume_mount = VolumeMount {
                name: "nix".to_string(),
                mount_path: "/nix".to_string(),
                ..Default::default()
            };
            volume_mounts.push(nix_volume_mount);
        }
        let mut envs = vec![];
        if container.origin == ContainerOrigin::DevelopmentEnvironment as i32 {
            let development_environment: DevelopmentEnvironment = development_environments::table.find(container_id).first(&mut db)?;
            envs.push(EnvVar {
                name: "CODE_SERVER_APP_NAME".to_string(),
                value: Some(format!("Atlas - {}", development_environment.name)),
                value_from: None,
            });
            let code_server_settings: Setting = settings::table.find("code_server_settings").first(&mut db)?;
            envs.push(EnvVar {
                name: "CODE_SERVER_SETTINGS".to_string(),
                value: Some(code_server_settings.value),
                value_from: None,
            });
            let code_server_extensions: Setting = settings::table.find("code_server_extensions").first(&mut db)?;
            envs.push(EnvVar {
                name: "CODE_SERVER_EXTENSIONS".to_string(),
                value: Some(code_server_extensions.value),
                value_from: None,
            });
        }
        let mut security_context = SecurityContext {
            run_as_user: Some(0),
            run_as_group: Some(0),
            ..Default::default()
        };
        let mut limits = BTreeMap::new();
        if container.origin == ContainerOrigin::DevelopmentEnvironment as i32 {
            let development_environment: DevelopmentEnvironment = development_environments::table.find(container_id).first(&mut db)?;
            if development_environment.nested_containers {
                security_context.proc_mount = Some("Unmasked".to_string());
                security_context.seccomp_profile = Some(SeccompProfile {
                    localhost_profile: None,
                    type_: "Unconfined".to_string(),
                });
                limits.insert("devic.es/tun".to_string(), Quantity("1".to_string()));
            }
        }
        let k8s_container = k8s_openapi::api::core::v1::Container {
            name: "main".to_string(),
            image: Some(container.image),
            command: Some(vec!["/usr/bin/sleep".to_string(), "infinity".to_string()]),
            volume_mounts: Some(volume_mounts),
            security_context: Some(security_context),
            env: Some(envs),
            resources: Some(ResourceRequirements {
                claims: None,
                limits: Some(limits),
                requests: None,
            }),
            ..Default::default()
        };
        let pull_secret = LocalObjectReference { name: "atlas-image-pull".to_string() };
        pod.spec = Some(PodSpec {
            share_process_namespace: Some(true),
            volumes: Some(volumes),
            containers: vec![k8s_container],
            image_pull_secrets: Some(vec![pull_secret]),
            host_users: Some(false),
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
        if pod_api.get_opt(&container_name).await?.is_some() {
            pod_api.delete(&container_name, &DeleteParams::default()).await?;
        }
        diesel::update(containers::table.find(container_id))
            .set(containers::state.eq(ContainerState::Stopped as i32))
            .execute(&mut db)?;
        notify_server("WEB:REFRESH").await?;
        notify_server("MASTER:CONTAINERS").await?;
    } else if container.state == ContainerState::Started as i32 && container.requested_state == ContainerState::Running as i32 {
        let pod_api: Api<Pod> = Api::namespaced(client, namespace);
        let binary = personalize_binary(&mut db, "x86_64", "linux").await?;
        let mut pod_started = false;
        for _ in 0..60 {
            let pod = pod_api.get(&container_name).await?;
            if pod.status.is_some() && pod.status.ok_or("Invalid pod")?.phase == Some("Running".to_string()) {
                pod_started = true;
                break;
            }
            sleep(Duration::from_secs(1)).await;
        }
        if !pod_started {
            diesel::update(containers::table.find(container_id))
                .set(containers::requested_state.eq(ContainerState::Stopped as i32))
                .execute(&mut db)?;
            notify_server("WEB:REFRESH").await?;
            notify_server("MASTER:CONTAINERS").await?;
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
                vec!["/usr/bin/sh", "-c", "cd / && (/usr/local/bin/atlas-agent supervise > /var/log/atlas.log 2>&1 &)"],
                &AttachParams::default(),
            )
            .await?;
        process.take_status().ok_or("Invalid process")?.await;
        process.join().await?;
        let mut device_found = false;
        for _ in 0..30 {
            let device: Option<Device> = devices::table.filter(devices::hostname.eq(&container_name)).first(&mut db).optional()?;
            if device.is_some() {
                device_found = true;
                break;
            }
            sleep(Duration::from_secs(1)).await;
        }
        if !device_found {
            diesel::update(containers::table.find(container_id))
                .set(containers::requested_state.eq(ContainerState::Stopped as i32))
                .execute(&mut db)?;
            notify_server("WEB:REFRESH").await?;
            notify_server("MASTER:CONTAINERS").await?;
            return Ok(());
        }
        let mut process = pod_api.exec(&container_name, vec!["/usr/local/bin/atlas-agent", "pubkey"], &AttachParams::default()).await?;
        process.take_status().ok_or("Invalid process")?.await;
        let mut pubkey = String::new();
        process.stdout().ok_or("Invalid process")?.read_to_string(&mut pubkey).await?;
        process.join().await?;
        let pubkey = BASE64_STANDARD.decode(pubkey.trim())?;
        let device: Device = devices::table.filter(devices::hostname.eq(container_name).and(devices::pubkey.eq(pubkey))).first(&mut db)?;
        diesel::update(devices::table.find(device.id.ok_or("Invalid device")?))
            .set((devices::container_id.eq(container_id), devices::approved.eq(true)))
            .execute(&mut db)?;
        diesel::update(containers::table.find(container_id))
            .set(containers::state.eq(ContainerState::Running as i32))
            .execute(&mut db)?;
        if container.origin == ContainerOrigin::DevelopmentEnvironment as i32 {
            let mut command = Command {
                id: None,
                device: device.id.ok_or("Invalid device")?,
                uid: Some(container.uid),
                code: PY_DEVELOPMENT_ENVIRONMENT.to_string(),
                completed: false,
                status_code: 0,
                stdout: "".to_owned(),
                stderr: "".to_owned(),
                requested_timestamp: SystemTime::now(),
                completed_timestamp: None,
                origin: CommandOrigin::SystemInitiated as i32,
                device_info_type: None,
                chat_id: None,
                chat_sequence: None,
                chat_tool_call_id: None,
                initiated: false,
                language: CommandLanguage::Python as i32,
            };
            command = diesel::insert_into(commands::table).values(command).get_result(&mut db)?;
            diesel::update(development_environments::table.find(container_id))
                .set(development_environments::code_server_password.eq(command.id.ok_or("Invalid command")?))
                .execute(&mut db)?;
        }
        notify_server("WEB:REFRESH").await?;
        notify_server("MASTER:CONTAINERS").await?;
    } else if container.state == ContainerState::Running as i32
        && (container.requested_state == ContainerState::Started as i32
            || container.requested_state == ContainerState::Stopped as i32
            || container.requested_state == ContainerState::NonExistent as i32)
    {
        let pod_api: Api<Pod> = Api::namespaced(client, namespace);
        if pod_api.get_opt(&container_name).await?.is_some() {
            let mut process = pod_api.exec(&container_name, vec!["/usr/bin/killall", "-9", "atlas-agent"], &AttachParams::default()).await?;
            process.take_status().ok_or("Invalid process")?.await;
            process.join().await?;
        }
        if container.origin == ContainerOrigin::DevelopmentEnvironment as i32 {
            diesel::update(development_environments::table.find(container_id))
                .set(development_environments::code_server_password.eq(None::<Uuid>))
                .execute(&mut db)?;
        }
        diesel::delete(devices::table.filter(devices::container_id.eq(container_id))).execute(&mut db)?;
        diesel::update(containers::table.find(container_id))
            .set(containers::state.eq(ContainerState::Started as i32))
            .execute(&mut db)?;
        notify_server("WEB:REFRESH").await?;
        notify_server("MASTER:CONTAINERS").await?;
    }
    Ok(())
}
