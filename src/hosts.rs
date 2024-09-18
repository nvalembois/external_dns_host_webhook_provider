use std::collections::{HashMap, HashSet};
use once_cell::sync::Lazy;
use regex::Regex;
use tracing::info;
use crate::config::CONFIG;

use kube::{api::{Api, Patch, PatchParams}, Client};
use k8s_openapi::api::core::v1::ConfigMap;
use serde_json::json;
use std::collections::BTreeMap;

static HOST_REGEXP: &str = r"(?m)^\s*(?P<address>[0-9\.:]+)\s+(?P<name>[A-Za-z0-9]([A-Za-z0-9-]{0,61}[A-Za-z0-9])?(\.[A-Za-z0-9]([A-Za-z0-9-]{0,61}[A-Za-z0-9])?)*)\s*$";

// return HashMap<name, ips>
pub async fn read_host() -> Result<HashMap<String,HashSet<String>>, kube::Error> {
    let mut records: HashMap<String,HashSet<String>> = HashMap::new();
    static RE: Lazy<Regex> = Lazy::new(|| Regex::new(&HOST_REGEXP).unwrap());

    // Création d'un client de l'APIServer avec la configuration par défaut (variables d'environnement ou fichiers)
    let client: Client = Client::try_default().await?;
    // Création d'une interface pour interroger les ConfigMap
    let configmaps: Api<ConfigMap> = Api::namespaced(client.clone(), &CONFIG.host_configmap_namespace);
    
    // Récupération de la config map conténant les données
    let cm: ConfigMap = configmaps.get(&CONFIG.host_configmap_name).await?;
    
    // Récupération du contenu du fichier host dans la clé du configmap
    let lines: String = match cm.data {
        Some(data) => {
            match data.get(&CONFIG.host_configmap_key) {
                Some(v) => v.to_string(),
                None => String::from(""),
            }
        },
        None => String::from(""),
    };

    // Parcourt chaque ligne du fichier hosts
    for line in lines.lines() {
        if let Some(parts) = RE.captures(&line) {
            // Extraction et conversion des captures en String
            let name = parts.name("name").unwrap().as_str().to_string();
            let address = parts.name("address").unwrap().as_str().to_string();

            // Ajout de l'adresse dans le vecteur correspondant à la clé 'name'
            records.entry(name)
                .or_insert_with(HashSet::new)
                .insert(address);
        } else {
            info!("Skip host line: {line}");
        }
    }

    // Renvoi le résultat
    Ok(records)
}

fn format_records(records: &HashMap<String,HashSet<String>>) -> String {
    records.iter().fold(String::new(), |mut acc, (name, ips)| {
        for ip in ips {
            acc.push_str(&format!("{ip} {name}\n"));
        }
        acc
    })
}

pub async fn write_host(records: &HashMap<String,HashSet<String>>) -> Result<(),kube::Error> {
    // Création d'un client de l'APIServer avec la configuration par défaut (variables d'environnement ou fichiers)
    let client: Client = Client::try_default().await?;
    // Création d'une interface pour interroger les ConfigMap
    let configmaps: Api<ConfigMap> = Api::namespaced(client.clone(), &CONFIG.host_configmap_namespace);
    
    // Création du dictionnaire des données à modifier
    let mut new_data = BTreeMap::new();
    new_data.insert(&CONFIG.host_configmap_key, format_records(&records)); // Exemple de modification

    // Créer un patch JSON pour modifier le ConfigMap
    let patch = json!({
        "data": new_data
    });

    // Paramètres de patch : On spécifie que c'est un merge patch
    static PATCH_PARAMS: Lazy<PatchParams> = Lazy::new(|| PatchParams::apply("external-dns-webhhok"));

    // Patcher le ConfigMap
    configmaps.patch(&CONFIG.host_configmap_name, &PATCH_PARAMS, &Patch::Merge(&patch)).await?;

    Ok(())
}
