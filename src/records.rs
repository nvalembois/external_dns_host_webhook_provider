use salvo::prelude::*;
use serde::{Serialize,Deserialize};
use tracing::info;
use core::str;
use std::collections::HashMap;

use crate::hosts::{read_host, write_host};

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
#[serde(rename_all(serialize = "camelCase", deserialize = "snake_case"))]
pub struct Endpoint {
	// The hostname of the DNS record
	pub dns_name: String,
	// The targets the DNS record points to
    pub targets: Targets,
	// RecordType type of record, e.g. CNAME, A, AAAA, SRV, TXT etc
	pub record_type: RecordType,
	// Identifier to distinguish multiple records with the same name and type (e.g. Route53 records with routing policies other than 'simple')
	pub set_identifier: String,
	// TTL for the record
	pub record_t_t_l: TTL,
	// Labels stores labels defined for the Endpoint
	pub labels: Option<Labels>,
	// ProviderSpecific stores provider specific config
	pub provider_specific: Option<ProviderSpecific>,
}

pub type Records = Vec<Endpoint>;

#[handler]
pub async fn get_records(res: &mut Response) {
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
pub async fn post_records(req: &mut Request, res: &mut Response) {
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
pub async fn post_adjustendpoints(req: &mut Request, res: &mut Response) {

    match req.take_body() {
        salvo::http::ReqBody::None=>info!("Empty body"),
        salvo::http::ReqBody::Once(body_bytes)=>{
            if let Ok(body_str) = str::from_utf8(&body_bytes) {
                info!("Corps de la requête : {}",body_str);
            } else {
                info!("Impossible de lire le corps de la requête en tant que texte UTF-8.");
            }},
        salvo::http::ReqBody::Hyper{inner,fusewire}=>info!("Hyper"),
        salvo::http::ReqBody::Boxed{inner,fusewire}=>info!("Boxed"),
        _ => info!("Other") };

    // Répondre à la requête après affichage
    res.render(Text::Plain("Corps de la requête reçu et affiché."));
    res.status_code(StatusCode::OK);
}
