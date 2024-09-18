use host_webhook_provider::config::CONFIG;
use host_webhook_provider::health::get_healthz;
use host_webhook_provider::records::{get_records, post_adjustendpoints, post_records};
use salvo::logging::Logger;
use salvo::server::ServerHandle;
use salvo::prelude::*;
use tokio::{signal, task};
use futures::future::join_all;
use std::time::Duration;
use tracing::{debug, error, info};

#[handler]
async fn get_root(req: &mut Request, res: &mut Response) {
    let domain_filter = CONFIG.domain_filter.clone();
    debug!("domain_filter: {:?}", &domain_filter);

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

    // Set Content-Type Header with Accept Header
    if let Some(v) = req.header("Accept") {
        let accept_header_value: String = v;
        if let Err(err) = res.add_header("Content-Type", accept_header_value, true) {
            res.status_code(StatusCode::BAD_REQUEST);
            res.render(Text::Plain(format!("Failed to add header Content-Type: {}",err.to_string())));
            return;
        };
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
    tracing_subscriber::fmt()
        .with_max_level(if CONFIG.debug {tracing::Level::DEBUG} else { tracing::Level::INFO} )
        .init();

    info!("Config: filters={}", &CONFIG.domain_filter.filters.join(","));
    info!("Config: exclude={}", &CONFIG.domain_filter.exclude.join(","));
    info!("Config: regex={}", &CONFIG.domain_filter.regex);
    info!("Config: regex_exclusion={}", &CONFIG.domain_filter.regex_exclusion);
    info!("Config: host_configmap_name={}", &CONFIG.host_configmap_name);
    info!("Config: host_configmap_namespace={}", &CONFIG.host_configmap_namespace);
    info!("Config: host_configmap_key={}", &CONFIG.host_configmap_key);
    info!("Config: listen_addr={}", &CONFIG.listen_addr);
    info!("Config: health_listen_addr={}", &CONFIG.health_listen_addr);
    info!("Config: dry_run={}", &CONFIG.dry_run);
    info!("Config: debug={}", &CONFIG.debug);

    // webhook
    let router_webhook = Router::new()
        .hoop(alter_content_type)
        .get(get_root)
        .push(Router::with_path("records").get(get_records).post(post_records))
        .push(Router::with_path("adjustendpoints").post(post_adjustendpoints));
    let service_webhook = Service::new(router_webhook)
        .hoop(Logger::new());
    let acceptor_webhook = TcpListener::new(&CONFIG.listen_addr)
        .bind().await;
    let server_webhook = Server::new(acceptor_webhook);
    
    // health
    let router_health = Router::new()
        .push(Router::with_path("healthz").get(get_healthz));
    let service_health = Service::new(router_health);
    let acceptor_health = TcpListener::new(&CONFIG.health_listen_addr)
        .bind().await;
    let server_health = Server::new(acceptor_health);

    // handle shutdown
    let mut handles: Vec<ServerHandle> = Vec::new();
    handles.push(server_webhook.handle());
    handles.push(server_health.handle());
    tokio::spawn(listen_shutdown_signal(handles));

    // start servers
    let task_webhook = task::spawn(async move {server_webhook.serve(service_webhook).await;});
    let task_health = task::spawn(async move {server_health.serve(service_health).await;});
    task_webhook.await.unwrap();
    task_health.await.unwrap();
}

async fn listen_shutdown_signal(handles: Vec<ServerHandle>) {
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

    async fn async_stop(handle: &ServerHandle) {
        handle.stop_graceful(Duration::from_secs(60*5));
    }

    let tasks: Vec<_> = handles.iter().map(|h| async_stop(h)).collect();
    _ = join_all(tasks).await;
}
