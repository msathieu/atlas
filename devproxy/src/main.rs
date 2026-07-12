use async_trait::async_trait;
use diesel::prelude::*;
use pingora::prelude::*;
use shared_api::print_startup;
use shared_server::{get_db_pool, models::Setting, schema::settings};
use std::sync::OnceLock;
use uuid::Uuid;

struct DevProxy;

static WEBSERVER_HOST: OnceLock<String> = OnceLock::new();

#[async_trait]
impl ProxyHttp for DevProxy {
    type CTX = Uuid;
    fn new_ctx(&self) -> Self::CTX {
        Uuid::nil()
    }
    async fn upstream_peer(&self, _session: &mut Session, ctx: &mut Self::CTX) -> Result<Box<HttpPeer>> {
        let upstream = HttpPeer::new(format!("atlas-dev-{}.atlas.svc.cluster.local:9000", ctx), false, "".to_string());
        Ok(Box::new(upstream))
    }
    async fn request_filter(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<bool> {
        let host = session.get_header("Host");
        let host = host.ok_or(Error::new_down(InvalidHTTPHeader))?.to_str().or(Err(Error::new_down(InvalidHTTPHeader)))?;
        let webserver_host = WEBSERVER_HOST.get().ok_or(Error::new(InternalError))?;
        let subdomain = host.strip_suffix(&format!(".dev.{}", webserver_host)).ok_or(Error::new_down(InvalidHTTPHeader))?;
        *ctx = Uuid::parse_str(subdomain).or(Err(Error::new_down(InvalidHTTPHeader)))?;
        Ok(false)
    }
}

fn main() {
    print_startup("devproxy", env!("CARGO_PKG_VERSION"));
    dotenvy::dotenv().ok();
    let pool = get_db_pool().unwrap();
    let mut db = pool.get().unwrap();
    let web_host: Setting = settings::table.find("webserver_host").get_result(&mut db).unwrap();
    WEBSERVER_HOST.set(web_host.value).unwrap();
    let mut server = Server::new(None).unwrap();
    server.bootstrap();
    let mut proxy = http_proxy_service(&server.configuration, DevProxy);
    proxy.add_tcp("0.0.0.0:8001");
    server.add_service(proxy);
    server.run_forever();
}
