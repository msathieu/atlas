#![forbid(unsafe_code)]

use async_trait::async_trait;
use bytes::Bytes;
use diesel::{
    prelude::*,
    r2d2::{ConnectionManager, Pool},
};
use pingora::{http::Method, prelude::*, server::ShutdownWatch, services::background::BackgroundService};
use redis::AsyncTypedCommands;
use shared_api::{ContainerState, print_startup};
use shared_server::{
    authenticate_jwt, get_db_pool, get_valkey_client, listen_for_restarts,
    models::{Command, Container, DevelopmentEnvironment, Setting},
    schema::{commands, containers, development_environments, settings},
};
use std::{net::ToSocketAddrs, sync::OnceLock};
use uuid::Uuid;

struct DevProxy;
struct RestartService;

static DB_POOL: OnceLock<Pool<ConnectionManager<PgConnection>>> = OnceLock::new();
static VALKEY_POOL: OnceLock<redis::aio::ConnectionManager> = OnceLock::new();

#[async_trait]
impl BackgroundService for RestartService {
    async fn start(&self, mut _shutdown: ShutdownWatch) {
        listen_for_restarts("DEVPROXY".to_string()).await;
    }
}
fn get_login_body(id: Uuid) -> Result<String> {
    let mut db = DB_POOL.get().ok_or(Error::new(InternalError))?.get().or(Err(Error::new(InternalError)))?;
    let development_environment: DevelopmentEnvironment = development_environments::table.find(id).first(&mut db).or(Err(Error::new_down(InvalidHTTPHeader)))?;
    let command: Command = commands::table
        .find(development_environment.code_server_password.ok_or(Error::new(InternalError))?)
        .first(&mut db)
        .or(Err(Error::new(InternalError)))?;
    if command.stdout.is_empty() {
        return Err(Error::new(InternalError));
    }
    Ok(format!("password={}", command.stdout))
}
#[async_trait]
impl ProxyHttp for DevProxy {
    type CTX = Uuid;
    fn new_ctx(&self) -> Self::CTX {
        Uuid::nil()
    }
    async fn upstream_peer(&self, _session: &mut Session, ctx: &mut Self::CTX) -> Result<Box<HttpPeer>> {
        let domain_name = format!("atlas-dev-{}.atlas.svc.cluster.local:9000", ctx);
        let address = domain_name
            .to_socket_addrs()
            .or(Err(Error::new_down(InvalidHTTPHeader)))?
            .next()
            .ok_or(Error::new_down(InvalidHTTPHeader))?;
        let upstream = HttpPeer::new(address, false, "".to_string());
        Ok(Box::new(upstream))
    }
    async fn request_filter(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<bool> {
        let mut db = DB_POOL.get().ok_or(Error::new(InternalError))?.get().or(Err(Error::new(InternalError)))?;
        let host = session.get_header("Host");
        let host = host.ok_or(Error::new_down(InvalidHTTPHeader))?.to_str().or(Err(Error::new_down(InvalidHTTPHeader)))?;
        let webserver_host: Setting = settings::table.find("webserver_host").first(&mut db).or(Err(Error::new(InternalError)))?;
        let subdomain = host.strip_suffix(&format!(".dev.{}", webserver_host.value)).ok_or(Error::new_down(InvalidHTTPHeader))?;
        let uuid = Uuid::parse_str(subdomain).or(Err(Error::new_down(InvalidHTTPHeader)))?;
        let container: Container = containers::table.find(uuid).first(&mut db).or(Err(Error::new_down(InvalidHTTPHeader)))?;
        let uri = &session.req_header().uri;
        if uri.path() == "/setup" {
            let mut response = ResponseHeader::build(307, None).or(Err(Error::new(InternalError)))?;
            let jwt = uri.query().ok_or(Error::new_down(InvalidHTTPHeader))?.strip_prefix("jwt=").ok_or(Error::new_down(InvalidHTTPHeader))?;
            authenticate_jwt(jwt, &format!("devproxy-{}", uuid), &mut db).or(Err(Error::new_down(InvalidHTTPHeader)))?;
            let cookie = format!("atlas_jwt={}; HttpOnly; Secure; SameSite=Strict", jwt);
            response.append_header("Set-Cookie", cookie).or(Err(Error::new(InternalError)))?;
            response.insert_header("Location", "/").or(Err(Error::new(InternalError)))?;
            session.write_response_header(Box::new(response), false).await.or(Err(Error::new(InternalError)))?;
            return Ok(true);
        } else {
            let mut authenticated = false;
            if let Some(cookie_header) = session.get_header("Cookie") {
                for header in cookie_header.to_str().or(Err(Error::new_down(InvalidHTTPHeader)))?.split(";") {
                    let header = header.trim();
                    if header.starts_with("atlas_jwt=") {
                        let jwt = header.strip_prefix("atlas_jwt=").ok_or(Error::new_down(InvalidHTTPHeader))?;
                        let auth_result = authenticate_jwt(jwt, &format!("devproxy-{}", uuid), &mut db);
                        if auth_result.is_ok() {
                            authenticated = true;
                        }
                        break;
                    }
                }
            }
            if !authenticated || container.state != ContainerState::Running as i32 {
                let mut response = ResponseHeader::build(307, None).or(Err(Error::new(InternalError)))?;
                response
                    .insert_header("Location", format!("https://{}/embed/development_environments/{}", webserver_host.value, uuid))
                    .or(Err(Error::new(InternalError)))?;
                session.write_response_header(Box::new(response), false).await.or(Err(Error::new(InternalError)))?;
                return Ok(true);
            }
        }
        *ctx = uuid;
        if VALKEY_POOL.get().is_none() {
            let client = get_valkey_client().await.or(Err(Error::new(InternalError)))?;
            VALKEY_POOL.set(client.0).ok();
        }
        let key = format!("DEVELOPMENT_ENVIRONMENT:{}", uuid);
        VALKEY_POOL
            .get()
            .ok_or(Error::new(InternalError))?
            .clone()
            .set_ex(key, true, 4 * 60 * 60)
            .await
            .or(Err(Error::new(InternalError)))?;
        Ok(false)
    }
    async fn upstream_request_filter(&self, session: &mut Session, upstream_request: &mut RequestHeader, ctx: &mut Self::CTX) -> Result<()> {
        let uri = &session.req_header().uri;
        if uri.path() == "/login" {
            upstream_request.set_method(Method::POST);
            upstream_request.insert_header("Content-Type", "application/x-www-form-urlencoded").or(Err(Error::new(InternalError)))?;
            upstream_request.insert_header("Content-Length", get_login_body(*ctx)?.len()).or(Err(Error::new(InternalError)))?;
        }
        Ok(())
    }
    async fn request_body_filter(&self, session: &mut Session, body: &mut Option<Bytes>, end_of_stream: bool, ctx: &mut Self::CTX) -> Result<()> {
        let uri = &session.req_header().uri;
        if uri.path() == "/login" && end_of_stream {
            *body = Some(Bytes::from(get_login_body(*ctx)?));
        }
        Ok(())
    }
}

fn main() {
    print_startup("devproxy", env!("CARGO_PKG_VERSION"));
    dotenvy::dotenv().ok();
    DB_POOL.set(get_db_pool().unwrap()).unwrap();
    let mut server = Server::new(None).unwrap();
    server.bootstrap();
    let mut proxy = http_proxy_service(&server.configuration, DevProxy);
    proxy.add_tcp("0.0.0.0:8001");
    let restart_service = background_service("restart", RestartService);
    server.add_service(proxy);
    server.add_service(restart_service);
    server.run_forever();
}
