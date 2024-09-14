use host_webhook_provider::config::CONFIG;
use host_webhook_provider::records::{get_records, post_adjustendpoints, post_records};
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

#[handler]
async fn get_root(req: &mut Request, res: &mut Response) {
    // Return error if Accept header has no value
    // Keep it's value to override Content-Type
    let accept_header_value: String = match req.header("Accept") {
        Option::Some(v) => { v }
        Option::None => {
            res.status_code(StatusCode::BAD_REQUEST);
            res.render(Text::Plain("Missing header Accept"));
            return;
        }
    };

    // 
    let domain_filter = CONFIG.domain_filter.clone();
    match serde_json::to_string(&domain_filter) {
        Ok(v) => {
            res.status_code(StatusCode::OK);
            res.render(Text::Json(v));
        }
        Err(e) => {
            error!("Erreur lors de la conversion en JSON: {}", e);
            res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
            res.render(Text::Plain("Erreur lors de la conversion en JSON"));
        }
    }
    
    // Override Content-Type Header with received Accept header value
    if let Err(err) = res.add_header("Content-Type", accept_header_value, true) {
        res.status_code(StatusCode::BAD_REQUEST);
        res.render(Text::Plain(format!("Failed to add header Content-Type: {}",err.to_string())));
        return;
    };
}

#[handler]
async fn alter_content_type(req: &mut Request) {
    if let Some(v) = req.header("Content-Type") {
        let content_type: String = v;
        if content_type == "application/external.dns.webhook+json;version=1" {
            if let Err(e) = req.add_header("Content-Type", "application/json;version=1", true) {
                info!("Failed to replace Content-Type: {:?}", e);
            } else if CONFIG.debug {
                debug!("modified content-type header application/external.dns.webhook+json;version=1 -> application/json;version=1")
            }
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();
    info!("Config: filters={}", &CONFIG.domain_filter.filters.join(","));
    info!("Config: exclude={}", &CONFIG.domain_filter.exclude.join(","));
    info!("Config: regex={}", &CONFIG.domain_filter.regex);
    info!("Config: regex_exclusion={}", &CONFIG.domain_filter.regex_exclusion);
    info!("Config: host_file_path={}", &CONFIG.host_file_path);
    info!("Config: listen_addr={}", &CONFIG.listen_addr);
    info!("Config: dry_run={}", &CONFIG.dry_run);
    info!("Config: debug={}", &CONFIG.debug);

    let router = Router::new()
        .hoop(alter_content_type)
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
