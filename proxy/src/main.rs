#![forbid(unsafe_code)]

use async_trait::async_trait;
use bytes::Bytes;
use diesel::{
    prelude::*,
    r2d2::{ConnectionManager, Pool},
};
use http::Uri;
use pingora::{
    http::Method,
    prelude::*,
    server::{ShutdownWatch, configuration::ServerConf},
    services::background::BackgroundService,
};
use redis::AsyncTypedCommands;
use serde_json::json;
use shared_api::{ContainerState, print_startup};
use shared_server::{
    authenticate_jwt, get_db_pool, get_valkey_client, listen_for_restarts,
    models::{Agent, Command, Container, DevelopmentEnvironment},
    schema::{agents, commands, containers, development_environments},
};
use std::{env, sync::OnceLock};
use tokio::net::lookup_host;
use uuid::Uuid;

#[derive(PartialEq, Clone, Copy)]
enum ProxyType {
    Unknown = 0,
    DevelopmentEnvironment = 1,
    Agent = 2,
}

struct Proxy;
struct RestartService;

struct ProxyContext {
    request_type: ProxyType,
    id: Uuid,
    port: Option<u16>,
    login_upstream: bool,
}

static DB_POOL: OnceLock<Pool<ConnectionManager<PgConnection>>> = OnceLock::new();
static VALKEY_POOL: OnceLock<redis::aio::ConnectionManager> = OnceLock::new();

#[async_trait]
impl BackgroundService for RestartService {
    async fn start(&self, mut _shutdown: ShutdownWatch) {
        listen_for_restarts("PROXY".to_string()).await;
    }
}
fn get_login_body(ctx: &ProxyContext) -> Result<String> {
    let mut db = DB_POOL.get().ok_or(Error::new(InternalError))?.get().or(Err(Error::new(InternalError)))?;
    if ctx.request_type == ProxyType::DevelopmentEnvironment {
        let development_environment: DevelopmentEnvironment = development_environments::table.find(ctx.id).first(&mut db).or(Err(Error::new_down(InvalidHTTPHeader)))?;
        let command: Command = commands::table
            .find(development_environment.command.ok_or(Error::new(InternalError))?)
            .first(&mut db)
            .or(Err(Error::new(InternalError)))?;
        if command.stdout.is_empty() || !command.stdout.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(Error::new(InternalError));
        }
        Ok(format!("password={}", command.stdout))
    } else if ctx.request_type == ProxyType::Agent {
        let agent: Agent = agents::table.find(ctx.id).first(&mut db).or(Err(Error::new_down(InvalidHTTPHeader)))?;
        let data = json!({"provider": "basic", "username": "admin", "password": agent.password});
        Ok(data.to_string())
    } else {
        Err(Error::new(InternalError))
    }
}
fn authenticate_proxy_jwt(request_type: ProxyType, port: Option<u16>, jwt: &str, uuid: Uuid) -> Result<()> {
    if request_type == ProxyType::DevelopmentEnvironment {
        if port.is_some() {
            authenticate_jwt(jwt, &format!("proxy-devproxy-{}", uuid), None, true).or(Err(Error::new_down(InvalidHTTPHeader)))?;
        } else {
            authenticate_jwt(jwt, &format!("proxy-dev-{}", uuid), None, true).or(Err(Error::new_down(InvalidHTTPHeader)))?;
        }
    } else if request_type == ProxyType::Agent {
        authenticate_jwt(jwt, &format!("proxy-agent-{}", uuid), None, true).or(Err(Error::new_down(InvalidHTTPHeader)))?;
    } else {
        return Err(Error::new(InternalError));
    }
    Ok(())
}
#[async_trait]
impl ProxyHttp for Proxy {
    type CTX = ProxyContext;
    fn new_ctx(&self) -> Self::CTX {
        ProxyContext {
            request_type: ProxyType::Unknown,
            id: Uuid::nil(),
            port: None,
            login_upstream: false,
        }
    }
    async fn upstream_peer(&self, _session: &mut Session, ctx: &mut Self::CTX) -> Result<Box<HttpPeer>> {
        match ctx.request_type {
            ProxyType::Unknown => Err(Error::new(InternalError)),
            ProxyType::DevelopmentEnvironment => {
                let domain_name = format!("atlas-dev-{}.atlas.svc.cluster.local:9000", ctx.id);
                let address = lookup_host(domain_name)
                    .await
                    .or(Err(Error::new_down(InvalidHTTPHeader)))?
                    .next()
                    .ok_or(Error::new_down(InvalidHTTPHeader))?;
                let upstream = HttpPeer::new(address, false, "".to_string());
                Ok(Box::new(upstream))
            }
            ProxyType::Agent => {
                let domain_name = format!("atlas-agent-{}.atlas.svc.cluster.local:9119", ctx.id);
                let address = lookup_host(domain_name)
                    .await
                    .or(Err(Error::new_down(InvalidHTTPHeader)))?
                    .next()
                    .ok_or(Error::new_down(InvalidHTTPHeader))?;
                let upstream = HttpPeer::new(address, false, "".to_string());
                Ok(Box::new(upstream))
            }
        }
    }
    async fn request_filter(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<bool> {
        let host = session.get_header("Host");
        let host = host.ok_or(Error::new_down(InvalidHTTPHeader))?.to_str().or(Err(Error::new_down(InvalidHTTPHeader)))?;
        let webserver_host = env::var("WEBSERVER_HOST").or(Err(Error::new(InternalError)))?;

        let mut uuid = None;
        let mut port: Option<u16> = None;
        let mut request_type = ProxyType::Unknown;
        if let Some(subdomain) = host.strip_suffix(&format!(".dev.{}", webserver_host)) {
            request_type = ProxyType::DevelopmentEnvironment;
            uuid = Some(Uuid::parse_str(subdomain).or(Err(Error::new_down(InvalidHTTPHeader)))?);
        } else if let Some(stripped_host) = host.strip_suffix(&format!(".devproxy.{}", webserver_host)) {
            request_type = ProxyType::DevelopmentEnvironment;
            let (uuid_str, port_str) = stripped_host.rsplit_once('-').ok_or(Error::new_down(InvalidHTTPHeader))?;
            uuid = Some(Uuid::parse_str(uuid_str).or(Err(Error::new_down(InvalidHTTPHeader)))?);
            let parsed_port = port_str.parse().or(Err(Error::new_down(InvalidHTTPHeader)))?;
            if parsed_port == 9000 {
                return Err(Error::new_down(InvalidHTTPHeader));
            }
            port = Some(parsed_port);
        } else if let Some(subdomain) = host.strip_suffix(&format!(".agent.{}", webserver_host)) {
            request_type = ProxyType::Agent;
            uuid = Some(Uuid::parse_str(subdomain).or(Err(Error::new_down(InvalidHTTPHeader)))?);
        }
        let uuid = uuid.ok_or(Error::new_down(InvalidHTTPHeader))?;

        let uri = &session.req_header().uri;
        if uri.path() == "/atlas_setup" {
            let mut response = ResponseHeader::build(307, None).or(Err(Error::new(InternalError)))?;
            let jwt = uri.query().ok_or(Error::new_down(InvalidHTTPHeader))?.strip_prefix("jwt=").ok_or(Error::new_down(InvalidHTTPHeader))?;
            authenticate_proxy_jwt(request_type, port, jwt, uuid)?;
            let cookie = format!("atlas_jwt={}; HttpOnly; Secure; SameSite=Strict", jwt);
            response.append_header("Set-Cookie", cookie).or(Err(Error::new(InternalError)))?;
            response.append_header("Set-Cookie", "code-server-session=; Max-Age=-1").or(Err(Error::new(InternalError)))?;
            response.insert_header("Location", "/").or(Err(Error::new(InternalError)))?;
            session.write_response_header(Box::new(response), false).await.or(Err(Error::new(InternalError)))?;
            return Ok(true);
        } else {
            let mut authenticated = false;
            if let Some(cookie_header) = session.get_header("Cookie") {
                for cookie in cookie_header.to_str().or(Err(Error::new_down(InvalidHTTPHeader)))?.split(";") {
                    let cookie = cookie.trim();
                    if cookie.starts_with("atlas_jwt=") {
                        let jwt = cookie.strip_prefix("atlas_jwt=").ok_or(Error::new_down(InvalidHTTPHeader))?;
                        if authenticate_proxy_jwt(request_type, port, jwt, uuid).is_ok() {
                            authenticated = true;
                        }
                        break;
                    }
                }
            }
            if port.is_none() {
                let mut db = DB_POOL.get().ok_or(Error::new(InternalError))?.get().or(Err(Error::new(InternalError)))?;
                let container: Container = containers::table.find(uuid).first(&mut db).or(Err(Error::new_down(InvalidHTTPHeader)))?;
                if container.state != ContainerState::Running as i32 {
                    authenticated = false;
                }
            }
            if !authenticated {
                let mut response = ResponseHeader::build(307, None).or(Err(Error::new(InternalError)))?;
                let mut query = "".to_string();
                if let Some(port) = port {
                    query = format!("?port={}", port);
                }
                if request_type == ProxyType::DevelopmentEnvironment {
                    response
                        .insert_header("Location", format!("https://{}/embed/development_environments/{}{}", webserver_host, uuid, query))
                        .or(Err(Error::new(InternalError)))?;
                } else if request_type == ProxyType::Agent {
                    response
                        .insert_header("Location", format!("https://{}/embed/agents/{}{}", webserver_host, uuid, query))
                        .or(Err(Error::new(InternalError)))?;
                }
                session.write_response_header(Box::new(response), false).await.or(Err(Error::new(InternalError)))?;
                return Ok(true);
            }
        }
        ctx.request_type = request_type;
        ctx.id = uuid;
        ctx.port = port;
        if ctx.port.is_none() {
            if VALKEY_POOL.get().is_none() {
                let client = get_valkey_client().await.or(Err(Error::new(InternalError)))?;
                VALKEY_POOL.set(client.0).ok();
            }
            let key = format!("DEVELOPMENT_ENVIRONMENT:{}", uuid);
            VALKEY_POOL
                .get()
                .ok_or(Error::new(InternalError))?
                .clone()
                .set_ex(key, true, 16 * 60 * 60)
                .await
                .or(Err(Error::new(InternalError)))?;
        }
        Ok(false)
    }
    async fn upstream_request_filter(&self, session: &mut Session, upstream_request: &mut RequestHeader, ctx: &mut Self::CTX) -> Result<()> {
        let uri = &session.req_header().uri;
        if ctx.request_type == ProxyType::DevelopmentEnvironment
            && let Some(port) = ctx.port
        {
            let upstream_host = format!("{}.localhost", port);
            upstream_request.insert_header("Host", &upstream_host).or(Err(Error::new(InternalError)))?;
            if let Some(upgrade_header) = session.get_header("Upgrade")
                && upgrade_header.to_str().or(Err(Error::new_down(InvalidHTTPHeader)))? == "websocket"
            {
                upstream_request.insert_header("Origin", format!("https://{}", upstream_host)).or(Err(Error::new(InternalError)))?;
            }
        }
        if let Some(cookie_header) = session.get_header("Cookie") {
            let mut new_cookie_header = String::new();
            for header in cookie_header.to_str().or(Err(Error::new_down(InvalidHTTPHeader)))?.split(";") {
                let header = header.trim();
                if !header.starts_with("atlas_jwt=") {
                    if !new_cookie_header.is_empty() {
                        new_cookie_header.push_str("; ");
                    }
                    new_cookie_header.push_str(header);
                }
            }
            upstream_request.insert_header("Cookie", new_cookie_header).or(Err(Error::new(InternalError)))?;
        }
        if ctx.request_type == ProxyType::DevelopmentEnvironment {
            let mut is_logged_in = false;
            if let Some(cookie_header) = session.get_header("Cookie") {
                for header in cookie_header.to_str().or(Err(Error::new_down(InvalidHTTPHeader)))?.split(";") {
                    let header = header.trim();
                    if header.starts_with("code-server-session=") {
                        is_logged_in = true;
                    }
                }
            }
            if (!is_logged_in || ctx.port.is_none()) && uri.path() == "/login" {
                upstream_request.set_method(Method::POST);
                upstream_request.insert_header("Content-Type", "application/x-www-form-urlencoded").or(Err(Error::new(InternalError)))?;
                upstream_request.insert_header("Content-Length", get_login_body(ctx)?.len()).or(Err(Error::new(InternalError)))?;
                ctx.login_upstream = true;
            }
        } else if ctx.request_type == ProxyType::Agent && uri.path() == "/login" {
            upstream_request.set_method(Method::POST);
            upstream_request.set_uri(Uri::from_static("/auth/password-login"));
            upstream_request.insert_header("Content-Type", "application/json").or(Err(Error::new(InternalError)))?;
            upstream_request.insert_header("Content-Length", get_login_body(ctx)?.len()).or(Err(Error::new(InternalError)))?;
            ctx.login_upstream = true;
        }
        Ok(())
    }
    async fn request_body_filter(&self, _session: &mut Session, body: &mut Option<Bytes>, end_of_stream: bool, ctx: &mut Self::CTX) -> Result<()> {
        if ctx.login_upstream && end_of_stream {
            *body = Some(Bytes::from(get_login_body(ctx)?));
        }
        Ok(())
    }
    async fn response_filter(&self, _session: &mut Session, upstream_response: &mut ResponseHeader, ctx: &mut Self::CTX) -> Result<()> {
        if ctx.request_type == ProxyType::Agent && ctx.login_upstream {
            upstream_response.insert_header("Location", "/").or(Err(Error::new(InternalError)))?;
            upstream_response.set_status(307).or(Err(Error::new(InternalError)))?;
        }
        Ok(())
    }
}

fn main() {
    print_startup("proxy", env!("CARGO_PKG_VERSION"));
    dotenvy::dotenv().ok();
    DB_POOL.set(get_db_pool().unwrap()).unwrap();
    let mut db = DB_POOL.get().unwrap().get().unwrap();
    authenticate_jwt("", "", Some(&mut db), false).ok(); // Prefetch JWT_KEY
    let mut server = Server::new_with_opt_and_conf(None, ServerConf { threads: 4, ..Default::default() });
    server.bootstrap();
    let mut proxy = http_proxy_service(&server.configuration, Proxy);
    proxy.add_tcp("0.0.0.0:8001");
    let restart_service = background_service("restart", RestartService);
    server.add_service(proxy);
    server.add_service(restart_service);
    server.run_forever();
}
