use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};
use core::str;
use std::collections::{HashMap, HashSet};

use crate::{config::CONFIG, hosts::{read_host, write_host}};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RecordType {
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
pub struct ProviderSpecificProperty {
    pub name: String,
	pub value: String,
}

pub type TTL = i64;
pub type ProviderSpecific = Vec<ProviderSpecificProperty>;
pub type Targets = Vec<String>;
pub type Labels = HashMap<String,String>;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all(serialize = "snake_case", deserialize = "camelCase"))]
pub struct Endpoint {
	// The hostname of the DNS record
	pub dns_name: String,
	// The targets the DNS record points to
    pub targets: Targets,
	// RecordType type of record, e.g. CNAME, A, AAAA, SRV, TXT etc
	pub record_type: RecordType,
	// Identifier to distinguish multiple records with the same name and type (e.g. Route53 records with routing policies other than 'simple')
	pub set_identifier: Option<String>,
	// TTL for the record
	pub record_t_t_l: Option<TTL>,
	// Labels stores labels defined for the Endpoint
	pub labels: Option<Labels>,
	// ProviderSpecific stores provider specific config
	pub provider_specific: Option<ProviderSpecific>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all(serialize = "snake_case", deserialize = "camelCase"))]
pub struct Changes {
	// Records that need to be created
	pub Create: Records,
	// Records that need to be updated (current data)
	UpdateOld: Records,
	// Records that need to be updated (desired data)
	UpdateNew: Records,
	// Records that need to be deleted
	Delete: Records,
}

pub type Records = Vec<Endpoint>;

#[handler]
pub async fn get_records(req: &mut Request, res: &mut Response) {
    // Variable à retourner
    let mut entrypoints: Vec<Endpoint> = Vec::new();
    for (name, ips) in read_host() {
        if CONFIG.debug {
            let mut msg = String::from("return record: ");
            msg += &name.clone();
            msg += " ";
            let mut first = true;
            for ip in &ips {
                if first { 
                    first = false; 
                } else {
                    msg += ",";
                }
                msg += &ip.clone();
            }
            debug!(msg);
        }
        entrypoints.push(Endpoint {
            dns_name: name.clone(),
            record_type: RecordType::A,
            targets: ips.into_iter().collect(),
            set_identifier: None,
            record_t_t_l: None,
            labels: None,
            provider_specific: None
        })
    }

    // Convertit les enregistrements en JSON et les envoie dans la réponse
    match serde_json::to_string(&entrypoints) {
        Ok(json) => {
            res.status_code(StatusCode::OK);
            res.render(Text::Json(json));
        }
        Err(e) => {
            eprintln!("Erreur lors de la conversion en JSON: {}", e);
            res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
            res.render(Text::Plain("Erreur lors de la conversion en JSON"));
            return;
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
pub async fn post_records(req: &mut Request, res: &mut Response) {
    // Récupérer le corps de la requête en tant que JSON
    let changes: Changes = match req.parse_json().await {
        Ok(records) => records,
        Err(_) => {
            res.status_code(StatusCode::BAD_REQUEST);
            res.render(Text::Plain("Invalid JSON input"));
            return;
        }
    };
    if CONFIG.debug {
        for r in &changes.Create {
            debug!("in create record: {:?}", r);
        }
        for r in &changes.Delete {
            debug!("in delete record: {:?}", r);
        }
        for r in &changes.UpdateNew {
            debug!("in update new record: {:?}", r);
        }
        for r in &changes.UpdateOld {
            debug!("in update old record: {:?}", r);
        }
    }

    // if !CONFIG.dry_run {
    //     // match write_host(&result) {
    //     //     Ok(_) => {
    //     //         res.status_code(StatusCode::OK);
    //     //         res.render(Text::Plain("success"));
    //     //     }
    //     //     Err(e) => {
    //     //         eprintln!("Erreur lors de l'ecriture du fichier hosts: {}", e);
    //     //         res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
    //     //         res.render(Text::Plain("Erreur lors de l'écriture du fichier hosts"));
    //     //         return;
    //     //     }
    //     // }
    // }
    
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
pub async fn post_adjustendpoints(req: &mut Request, res: &mut Response) {
    let mut records: Records = match req.parse_json().await {
        Ok(records) => records,
        Err(e) => {
            info!("Impossible de lire le corps de la requête en tant que texte UTF-8 : {}.", e);
            res.render(Text::Plain(e.to_string()));
            res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
            return;
        }
    };
    if CONFIG.debug {
        for r in &records {
            debug!("in record: {:?}", r);
        }
    }

    for record in &mut records {
        record.set_identifier = None;
        record.record_t_t_l = None;
        record.labels = None;
        record.provider_specific = None;
    }

    if CONFIG.debug {
        for r in &records {
            debug!("out record: {:?}", r);
        }
    }

    match serde_json::to_string(&records) {
        Ok(json) => {
            res.status_code(StatusCode::OK);
            res.render(Text::Json(json));
        }
        Err(e) => {
            eprintln!("Erreur lors de la conversion en JSON: {}", e);
            res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
            res.render(Text::Plain("Erreur lors de la conversion en JSON"));
            return;
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
