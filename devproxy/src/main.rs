#![forbid(unsafe_code)]

use async_trait::async_trait;
use diesel::{
    prelude::*,
    r2d2::{ConnectionManager, Pool},
};
use pingora::{prelude::*, server::ShutdownWatch, services::background::BackgroundService};
use shared_api::print_startup;
use shared_server::{authenticate_jwt, get_db_pool, listen_for_restarts, models::Setting, schema::settings};
use std::{net::ToSocketAddrs, sync::OnceLock};
use uuid::Uuid;

struct DevProxy;
struct RestartService;

static WEBSERVER_HOST: OnceLock<String> = OnceLock::new();
static DB_POOL: OnceLock<Pool<ConnectionManager<PgConnection>>> = OnceLock::new();

#[async_trait]
impl BackgroundService for RestartService {
    async fn start(&self, mut _shutdown: ShutdownWatch) {
        listen_for_restarts("DEVPROXY".to_string()).await;
    }
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
        let host = session.get_header("Host");
        let host = host.ok_or(Error::new_down(InvalidHTTPHeader))?.to_str().or(Err(Error::new_down(InvalidHTTPHeader)))?;
        let webserver_host = WEBSERVER_HOST.get().ok_or(Error::new(InternalError))?;
        let subdomain = host.strip_suffix(&format!(".dev.{}", webserver_host)).ok_or(Error::new_down(InvalidHTTPHeader))?;
        let uuid = Uuid::parse_str(subdomain).or(Err(Error::new_down(InvalidHTTPHeader)))?;
        let uri = &session.req_header().uri;
        if uri.path() == "/setup" {
            let mut response = ResponseHeader::build(307, None).or(Err(Error::new(InternalError)))?;
            let jwt = uri.query().ok_or(Error::new_down(InvalidHTTPHeader))?.strip_prefix("jwt=").ok_or(Error::new_down(InvalidHTTPHeader))?;
            let mut db = DB_POOL.get().ok_or(Error::new(InternalError))?.get().or(Err(Error::new(InternalError)))?;
            authenticate_jwt(jwt, &format!("devproxy-{}", uuid), &mut db).or(Err(Error::new_down(InvalidHTTPHeader)))?;
            let cookie = format!("atlas_jwt={}; HttpOnly; Secure; SameSite=Strict", jwt);
            response.append_header("Set-Cookie", cookie).or(Err(Error::new(InternalError)))?;
            response.insert_header("Location", "/").or(Err(Error::new(InternalError)))?;
            session.write_response_header(Box::new(response), false).await.or(Err(Error::new(InternalError)))?;
            return Ok(true);
        } else {
            let mut authenticated = false;
            for header in session
                .get_header("Cookie")
                .ok_or(Error::new_down(InvalidHTTPHeader))?
                .to_str()
                .or(Err(Error::new_down(InvalidHTTPHeader)))?
                .split(";")
            {
                let header = header.trim();
                if header.starts_with("atlas_jwt=") {
                    let jwt = header.strip_prefix("atlas_jwt=").ok_or(Error::new_down(InvalidHTTPHeader))?;
                    let mut db = DB_POOL.get().ok_or(Error::new(InternalError))?.get().or(Err(Error::new(InternalError)))?;
                    authenticate_jwt(jwt, &format!("devproxy-{}", uuid), &mut db).or(Err(Error::new_down(InvalidHTTPHeader)))?;
                    authenticated = true;
                    break;
                }
            }
            if !authenticated {
                return Err(Error::new_down(InvalidHTTPHeader));
            }
        }
        *ctx = uuid;
        Ok(false)
    }
}

fn main() {
    print_startup("devproxy", env!("CARGO_PKG_VERSION"));
    dotenvy::dotenv().ok();
    DB_POOL.set(get_db_pool().unwrap()).unwrap();
    let mut db = DB_POOL.get().unwrap().get().unwrap();
    let web_host: Setting = settings::table.find("webserver_host").first(&mut db).unwrap();
    WEBSERVER_HOST.set(web_host.value).unwrap();
    let mut server = Server::new(None).unwrap();
    server.bootstrap();
    let mut proxy = http_proxy_service(&server.configuration, DevProxy);
    proxy.add_tcp("0.0.0.0:8001");
    let restart_service = background_service("restart", RestartService);
    server.add_service(proxy);
    server.add_service(restart_service);
    server.run_forever();
}
