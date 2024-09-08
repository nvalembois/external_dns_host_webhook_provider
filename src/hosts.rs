use std::fs::File;
use std::io::{BufRead, BufReader, Write};

use crate::config::CONFIG;
use crate::records::{Endpoint, Labels, ProviderSpecific, RecordType, Records, TTL};


pub fn read_host() -> Records {
    let mut records = Records::new();

    // Ouvre le fichier hosts en lecture
    let file = match File::open(&CONFIG.host_file_path) {
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

pub fn write_host(records: &Records) -> std::io::Result<()> {
    // Ouvre le fichier hosts en lecture
    let mut file = std::fs::OpenOptions::new().write(true).open(&CONFIG.host_file_path)?;
    for record in records {
        for ip in &record.targets {
            let entry = format!("{ip} {}\n", record.dns_name);
            file.write_all(entry.as_bytes())?;
        }
    }
    file.flush()?;
    Ok(())
}
