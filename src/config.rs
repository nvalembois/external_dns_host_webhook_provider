use std::sync::Mutex;
use once_cell::sync::Lazy;
use clap::Parser;
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Config {
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
    #[arg(long, value_name = "DNS_PREFIX", env = "DNS_PREFIX")]
    pub dns_prefix: String,
    #[arg(long, value_name = "HOST_FILE_PATH", env = "HOST_FILE_PATH", default_value_t = String::from("/etc/hosts"))]
    pub host_file_path: String,
    #[arg(long, value_name = "LISTEN_ADDR", env = "LISTEN_ADDR", default_value_t = String::from("127.0.0.1:8888"))]
    pub listen_addr: String,
}
pub static CONFIG: Lazy<Config> = Lazy::new(|| {Config::parse()});

pub type Domains = Vec<String>;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all(serialize = "camelCase", deserialize = "snake_case"))]
pub struct DomainFilter {
    // Filters define what domains to match
    pub filters: Domains,
    // exclude define what domains not to match
    pub exclude: Domains,
    // regex defines a regular expression to match the domains
    pub regex: String,
    // regexExclusion defines a regular expression to exclude the domains matched
    pub regex_exclusion: String,
}

pub static DOMAIN_FILTER: Lazy<DomainFilter> = Lazy::new(|| {
    DomainFilter {
        filters: vec![],
        exclude: vec![],
        regex: String::from(""),
        regex_exclusion: String::from("")
    }
});