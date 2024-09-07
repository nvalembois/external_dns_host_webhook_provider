use salvo::prelude::*;
use salvo::server::ServerHandle;
use serde::{Serialize,Deserialize};
use tokio::signal;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::time::Duration;
use clap::{Parser};
use std::sync::{Arc, Mutex}; // Importer Arc et Mutex

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Provider {
    #[arg(long, default_value_t = false)]
    dry_run: bool,
    #[arg(long, value_name = "DNS_PREFIX", env = "DNS_PREFIX")]
    dns_prefix: String,
    #[arg(skip = DomainFilter {filters: vec![], exclude: vec![], regex: String::from(""), regex_exclusion: String::from("") })]
    domain_filter: DomainFilter,
    #[arg(long, value_name = "HOST_FILE_PATH", env = "HOST_FILE_PATH")]
    host_file_path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
enum RecordType {
    A,
    AAAA,
    CNAME,
    TXT,
    SRV,
    NS,
    PTR,
    MX,
    NAPTR
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ProviderSpecificProperty {
    name: String,
	value: String,
}

type TTL = i64;
type ProviderSpecific = Vec<ProviderSpecificProperty>;
type Targets = Vec<String>;
type Labels = HashMap<String,String>;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all(serialize = "camelCase", deserialize = "snake_case"))]
struct Endpoint {
	// The hostname of the DNS record
	dns_name: String,
	// The targets the DNS record points to
    targets: Targets,
	// RecordType type of record, e.g. CNAME, A, AAAA, SRV, TXT etc
	record_type: RecordType,
	// Identifier to distinguish multiple records with the same name and type (e.g. Route53 records with routing policies other than 'simple')
	set_identifier: String,
	// TTL for the record
	record_t_t_l: TTL,
	// Labels stores labels defined for the Endpoint
	labels: Option<Labels>,
	// ProviderSpecific stores provider specific config
	provider_specific: Option<ProviderSpecific>,
}

type Records = Vec<Endpoint>;

type Filters = Vec<String>;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all(serialize = "camelCase", deserialize = "snake_case"))]
struct DomainFilter {
    // Filters define what domains to match
    filters: Filters,
    // exclude define what domains not to match
    exclude: Vec<String>,
    // regex defines a regular expression to match the domains
    regex: String,
    // regexExclusion defines a regular expression to exclude the domains matched
    regex_exclusion: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all(serialize = "camelCase", deserialize = "snake_case"))]
struct Changes {
    create: Vec<Endpoint>,
    update_old: Vec<Endpoint>,
    update_new: Vec<Endpoint>,
    delete: Vec<Endpoint>,
}

fn read_host() -> Records {
    let hosts_file_path = "hosts";
    let mut records = Records::new();

    // Ouvre le fichier hosts en lecture
    let file = match File::open(hosts_file_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Erreur lors de l'ouverture du fichier hosts: {}", e);
            return records;
        }
    };

    let reader = BufReader::new(file);

    // Parcourt chaque ligne du fichier hosts
    for line in reader.lines() {
        match line {
            Ok(l) => {
                let parts: Vec<&str> = l.split_whitespace().collect();
                if parts.len() >= 2 {
                    let dns_name = parts[0].to_string();
                    let targets = vec![parts[1].to_string()];
                    let record_type: RecordType = RecordType::A;
                    let set_identifier: String = String::from("");
                    let record_t_t_l: TTL = 0;
                    let labels: Option<Labels> = Option::None;
                    let provider_specific: Option<ProviderSpecific>=Option::None;
                    records.push(Endpoint { dns_name, targets, record_type, set_identifier, record_t_t_l, labels, provider_specific });
                }
            }
            Err(e) => {
                eprintln!("Erreur lors de la lecture du fichier hosts: {}", e);
            }
        }
    }

    records
}

fn write_host(records: &Records) -> std::io::Result<()> {
    let hosts_file_path = "hosts";
    // Ouvre le fichier hosts en lecture
    let mut file = std::fs::OpenOptions::new().write(true).open(hosts_file_path)?;
    for record in records {
        for ip in &record.targets {
            let entry = format!("{ip} {}\n", record.dns_name);
            file.write_all(entry.as_bytes())?;
        }
    }
    file.flush()?;
    Ok(())
}

#[handler]
async fn get_records(res: &mut Response, config: Arc<Mutex<Provider>>) {
    // Lit le fichier hosts
    let records = read_host();

    // Convertit les enregistrements en JSON et les envoie dans la réponse
    match serde_json::to_string(&records) {
        Ok(json) => {
            res.status_code(StatusCode::OK);
            res.render(Text::Json(json));
        }
        Err(e) => {
            eprintln!("Erreur lors de la conversion en JSON: {}", e);
            res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
            res.render(Text::Plain("Erreur lors de la conversion en JSON"));
        }
    }
}

#[handler]
async fn post_records(req: &mut Request, res: &mut Response, config: Arc<Mutex<Provider>>) {
    // Récupérer le corps de la requête en tant que JSON
    let new_records: Records = match req.parse_json().await {
        Ok(records) => records,
        Err(_) => {
            res.status_code(StatusCode::BAD_REQUEST);
            res.render(Text::Plain("Invalid JSON input"));
            return;
        }
    };

    // Chemin vers le fichier hosts
    let records = read_host();
    let mut result = Records::new();
    let mut changed = false;

    // Parcours des enregistrements DNS
    'new_records: for new_record in &new_records {
        for record in &records {
            if record.dns_name == new_record.dns_name {
                changed = true;
                result.push(new_record.clone());
                continue 'new_records;
            } else {
                result.push(record.clone());
            }
        }
    }

    if ! changed { 
        res.status_code(StatusCode::OK);
        res.render(Text::Plain("Nothing to do"));
        return;
    }
    match write_host(&result) {
        Ok(_) => {
            res.status_code(StatusCode::OK);
            res.render(Text::Plain("success"));
        }
        Err(e) => {
            eprintln!("Erreur lors de l'ecriture du fichier hosts: {}", e);
            res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
            res.render(Text::Plain("Erreur lors de l'écriture du fichier hosts"));
        }
    }
}

#[handler]
async fn post_adjustendpoints(_req: &mut Request, res: &mut Response) {
    let hosts_file_path = "hosts";

    // Ouvre le fichier hosts en lecture
    let file = match File::open(hosts_file_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Erreur lors de l'ouverture du fichier hosts: {}", e);
            res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
            res.render(Text::Plain("Erreur lors de l'ouverture du fichier hosts"));
            return;
        }
    };

    let reader = BufReader::new(file);
    let mut records: Vec<Endpoint> = Vec::new();

    // Parcourt chaque ligne du fichier hosts
    for line in reader.lines() {
        match line {
            Ok(l) => {
                let parts: Vec<&str> = l.split_whitespace().collect();
                if parts.len() >= 2 {
                    let dns_name = parts[0].to_string();
                    let targets = vec![parts[1].to_string()];
                    let record_type: RecordType = RecordType::A;
                    let set_identifier: String = String::from("");
                    let record_t_t_l: TTL = 0;
                    let labels: Option<Labels> = Option::None;
                    let provider_specific: Option<ProviderSpecific>=Option::None;
                    records.push(Endpoint { dns_name, targets, record_type, set_identifier, record_t_t_l, labels, provider_specific });
                }
            }
            Err(e) => {
                eprintln!("Erreur lors de la lecture du fichier hosts: {}", e);
                res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
                res.render(Text::Plain("Erreur lors de la lecture du fichier hosts"));
                return;
            }
        }
    }

    // Convertit les enregistrements en JSON et les envoie dans la réponse
    match serde_json::to_string(&records) {
        Ok(json) => {
            res.status_code(StatusCode::OK);
            res.render(Text::Json(json));
        }
        Err(e) => {
            eprintln!("Erreur lors de la conversion en JSON: {}", e);
            res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
            res.render(Text::Plain("Erreur lors de la conversion en JSON"));
        }
    }
}

#[handler]
async fn get_healthz(res: &mut Response) {
    res.status_code(StatusCode::OK);
}

#[handler]
async fn get_root(req: &mut Request, res: &mut Response, config: Arc<Mutex<Provider>>) {
    // Récupérer le corps de la requête en tant que JSON
    let domain_filter: DomainFilter = match req.parse_json().await {
        Ok(domain_filter) => domain_filter,
        Err(_) => {
            res.status_code(StatusCode::BAD_REQUEST);
            res.render(Text::Plain("Invalid JSON input"));
            return;
        }
    };
    let mut config_guard = config.lock().unwrap();
    config_guard.domain_filter = domain_filter;

    let value: String = match req.header("Accept") {
        Option::Some(value) => { value }
        Option::None => {
            res.status_code(StatusCode::BAD_REQUEST);
            res.render(Text::Plain("Missing header Accept"));
            return;
        }
    };
    
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
    let config = Arc::new(Mutex::new(Provider::parse()));
    let router = Router::new()
        .get(get_root)
        .push(Router::with_path("records").get(get_records).post(get_records))
        .push(Router::with_path("adjustendpoints").post(post_adjustendpoints))
        .push(Router::with_path("healthz").get(get_healthz));

    let acceptor = TcpListener::new("127.0.0.1:8888").bind().await;
    let server = Server::new(acceptor);
    let handle = server.handle();
    // Listen Shutdown Signal
    tokio::spawn(listen_shutdown_signal(handle));
    server.serve(router).await;
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
