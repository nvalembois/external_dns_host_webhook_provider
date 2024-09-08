use host_webhook_provider::config::{DomainFilter, DOMAIN_FILTER, CONFIG};
use host_webhook_provider::records::{get_records, post_adjustendpoints, post_records};
use salvo::logging::Logger;
use salvo::server::ServerHandle;
use salvo::prelude::*;
use tokio::signal;
use std::time::Duration;
use tracing::{info,error};

#[handler]
async fn get_healthz(res: &mut Response) {
    res.render(Text::Plain("Ok!"));
    res.status_code(StatusCode::OK);
}

#[handler]
async fn get_root(req: &mut Request, res: &mut Response) {
    // Récupérer le corps de la requête en tant que JSON
    // let new_domain_filter: DomainFilter = match req.parse_json().await {
    //     Ok(domain_filter) => domain_filter,
    //     Err(_) => {
    //         error!("invalid input");
    //         res.status_code(StatusCode::BAD_REQUEST);
    //         res.render(Text::Plain("Invalid JSON input"));
    //         return;
    //     }
    // };
    // {
    //     let mut domain_filter = DOMAIN_FILTER.lock().unwrap();
    //     *domain_filter = new_domain_filter;
    // }
    let value: String = match req.header("Accept") {
        Option::Some(value) => { value }
        Option::None => {
            res.status_code(StatusCode::BAD_REQUEST);
            res.render(Text::Plain("Missing header Accept"));
            return;
        }
    };

    let domain_filter = DOMAIN_FILTER.clone();
    match serde_json::to_string(&domain_filter) {
        Ok(json) => {
            res.status_code(StatusCode::OK);
            res.render(Text::Json(json));
        }
        Err(e) => {
            error!("Erreur lors de la conversion en JSON: {}", e);
            res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
            res.render(Text::Plain("Erreur lors de la conversion en JSON"));
        }
    }
    
    match res.add_header("Content-Type", value, true) {
        Ok (_) => {}
        Err(err) => {
            res.status_code(StatusCode::BAD_REQUEST);
            res.render(Text::Plain(format!("Failed to add header Content-Type: {}",err.to_string())));
            return;
        }
    };
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();
    info!("Config: dry_run={}", &CONFIG.dry_run);
    info!("Config: dns_prefix={}", &CONFIG.dns_prefix);
    info!("Config: host_file_path={}", &CONFIG.host_file_path);
    info!("Config: listen_addr={}", &CONFIG.listen_addr);

    let router = Router::new()
        .get(get_root)
        .push(Router::with_path("records").get(get_records).post(post_records))
        .push(Router::with_path("adjustendpoints").post(post_adjustendpoints))
        .push(Router::with_path("healthz").get(get_healthz));
    let service = Service::new(router)
        .hoop(Logger::new());
    let acceptor = TcpListener::new(&CONFIG.listen_addr)
        .bind().await;
    let server = Server::new(acceptor);
    let handle = server.handle();
    // Listen Shutdown Signal
    tokio::spawn(listen_shutdown_signal(handle));
    server.serve(service).await;
}

async fn listen_shutdown_signal(handle: ServerHandle) {
    // Wait Shutdown Signal
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(windows)]
    let terminate = async {
        signal::windows::ctrl_c()
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => println!("ctrl_c signal received"),
        _ = terminate => println!("terminate signal received"),
    };

    // Graceful Shutdown Server
    handle.stop_graceful(Duration::from_secs(60*5));
}
