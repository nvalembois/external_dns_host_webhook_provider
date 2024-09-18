use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};
use core::str;
use std::collections::HashMap;

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
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "PascalCase")]
pub struct Changes {
	// Records that need to be created
	pub create: Option<Records>,
	// Records that need to be updated (current data)
	pub update_old: Option<Records>,
	// Records that need to be updated (desired data)
	pub update_new: Option<Records>,
	// Records that need to be deleted
	pub delete: Option<Records>,
}

pub type Records = Vec<Endpoint>;

#[handler]
pub async fn get_records(req: &mut Request, res: &mut Response) {
    // Variable à retourner
    let mut entrypoints: Vec<Endpoint> = Vec::new();
    let records = match read_host().await {
        Ok(v) => v,
        Err(e) => {
            error!("Erreur de récupération des données {e}");
            res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
            res.render(Text::Plain(format!("Erreur de récupération des données {e}")));
            return;
        }
    };

    for (name, ips) in records {
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
        Err(e) => {
            warn!("ParseError: {e}");
            match req.payload().await {
                Ok(b) => { match str::from_utf8(b) {
                        Ok(s) => {warn!("body: {s}");}
                        Err(e) => {warn!("body_from_utf8 err: {e}");}
                    }}
                Err(e) => {warn!("get body {e}");}
            };
            res.status_code(StatusCode::BAD_REQUEST);
            res.render(Text::Plain("Invalid JSON input"));
            return;
        }
    };
    if CONFIG.debug {
        if let Some(r) = &changes.create {
            debug!("in create records: {:?}", r);
        }
        if let Some(r) = &changes.delete {
            debug!("in delete records: {:?}", r);
        }
        if let Some(r) = &changes.update_new {
            debug!("in update new records: {:?}", r);
        }
        if let Some(r) = &changes.update_old {
            debug!("in update old records: {:?}", r);
        }
    }

    if !CONFIG.dry_run {
        let mut host_records= match read_host().await {
            Ok(v) => v,
            Err(e) => {
                error!("Erreur de récupération des données {e}");
                res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
                res.render(Text::Plain(format!("Erreur de récupération des données {e}")));
                return;
            }
        };

        // Add create items to records
        if let Some(records) = &changes.create {
            for record in records {
                if let Some(e) = host_records.insert(
                                        record.dns_name.clone(),
                                        record.targets.clone().into_iter().collect()) {
                    warn!("create replaced: {e:?}");
                }
            }
        }
        // Remove delete items
        if let Some(records) = &changes.delete {
            for record in records {
                match host_records.remove(&record.dns_name) {
                    Some(v) => { 
                        if CONFIG.debug {
                            let ips: Vec<String> = v.into_iter().collect();
                            debug!("removed {} -> {}", record.dns_name, ips.join(",")); }}
                    None => { warn!("delete {} isn't in hosts", record.dns_name); }
                }
            }
        }
        // Replace from update_old by update_new
        if let Some(new_records) = &changes.update_new {
            if let Some(old_records) = &changes.update_old {
                let mut old_record_iter = old_records.iter();
                for new_record in new_records {
                   if let Some(old_record) = old_record_iter.next() {
                        if old_record.dns_name != new_record.dns_name {
                            warn!("skip replace for records {:?} -> {:?}", old_record, new_record);
                            continue;
                        }
                        match host_records.get_mut(&old_record.dns_name) {
                            Some(ips) => {
                                ips.retain(|ip| !old_record.targets.contains(&ip));
                                for ip in &new_record.targets {
                                    if !ips.insert(ip.clone()) {
                                        warn!("hosts {} all_ready contains {ip}", new_record.dns_name);
                                    }
                                }
                            }
                            None => { warn!("replace {} isn't in hosts", &old_record.dns_name);
                            if let Some(e) = host_records.insert(
                                new_record.dns_name.clone(),
                                new_record.targets.clone().into_iter().collect()) {
                                    warn!("cannot add {new_record:?} : {e:?}");
                                }
                            }
                        }
                    } else {
                        warn!("Cannot iterate on old_records");
                    }
                }
            } else {
                warn!("No changes.OldRecords and Some(changes.NewRecords)");
            }
        } else if let Some(_) = &changes.update_old {
            warn!("No changes.NewRecords and Some(changes.OldRecords)");
        }
        // Finaly replace hosts
        if let Err(e) = write_host(&host_records).await {
            error!("Failed to write host file : {e}");
            res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
            res.render(Text::Plain(format!("Failed to patch configmap : {e}")));
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
    res.status_code(StatusCode::NO_CONTENT);
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

    // let body = match req.payload().await {
    //     Ok(b) => { match str::from_utf8(b) {
    //             Ok(s) => s.to_string(),
    //             Err(e) => e.to_string(),
    //         }}
    //     Err(e) => e.to_string(),
    // };
    // debug!("set response: {body}");
    // res.render(Text::Plain(body));
    // res.status_code(StatusCode::OK);

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
