use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use once_cell::sync::Lazy;
use regex::Regex;
use tracing::{info,error};
use crate::config::CONFIG;

static HOST_REGEXP: &str = r"(?m)^\s*(?P<address>[0-9\.:]+)\s+(?P<name>[A-Za-z0-9]([A-Za-z0-9-]{0,61}[A-Za-z0-9])?(\.[A-Za-z0-9]([A-Za-z0-9-]{0,61}[A-Za-z0-9])?)*)\s*$";

// return HashMap<name, ips>
pub fn read_host() -> HashMap<String,HashSet<String>> {
    let mut records: HashMap<String,HashSet<String>> = HashMap::new();
    static RE: Lazy<Regex> = Lazy::new(|| Regex::new(&HOST_REGEXP).unwrap());

    // Ouvre le fichier hosts en lecture
    let file = match File::open(&CONFIG.host_file_path) {
        Ok(f) => f,
        Err(e) => {
            error!("Erreur lors de l'ouverture du fichier hosts: {}", e);
            return records;
        }
    };

    let reader = BufReader::new(file);

    // Parcourt chaque ligne du fichier hosts
    for line in reader.lines() {
        match line {
            Ok(l) => {
                if let Some(parts) = RE.captures(&l) {
                    // Extraction et conversion des captures en String
                    let name = parts.name("name").unwrap().as_str().to_string();
                    let address = parts.name("address").unwrap().as_str().to_string();

                    // Ajout de l'adresse dans le vecteur correspondant à la clé 'name'
                    records.entry(name)
                        .or_insert_with(HashSet::new)
                        .insert(address);
                } else {
                    info!("Skip host line: {l}");
                }
            }
            Err(e) => {
                error!("Erreur lors de la lecture du fichier hosts: {}", e);
            }
        }
    }
    records
}

pub fn write_host(records: &HashMap<String,HashSet<String>>) -> std::io::Result<()> {
    // Ouvre le fichier hosts en lecture
    let mut file = std::fs::OpenOptions::new().write(true).create(true).open(&CONFIG.host_file_path)?;
    for (name, ips)  in records {
        for ip in ips {
            file.write_all(format!("{ip} {name}\n").as_bytes())?;
        }
    }
    file.flush()?;
    Ok(())
}
