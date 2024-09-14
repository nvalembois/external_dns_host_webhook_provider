use crate::config::CONFIG;
use salvo::logging::Logger;
use salvo::server::ServerHandle;
use salvo::prelude::*;
use tokio::signal;
use std::time::Duration;
use tracing::{debug, error, info};

#[handler]
async fn get_healthz(res: &mut Response) {
    res.render(Text::Plain("Ok!"));
    res.status_code(StatusCode::OK);
}

#[tokio::main]
pub async fn health_server() -> ServerHandle{
    let router = Router::new()
        .push(Router::with_path("healthz").get(get_healthz));
    let service = Service::new(router)
        .hoop(Logger::new());
    let acceptor = TcpListener::new(&CONFIG.health_listen_addr)
        .bind().await;
    let server = Server::new(acceptor);
    let handle = server.handle();
    server.serve(service).await;
    handle
}
