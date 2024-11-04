use std::fs;
use toml;

pub struct Config {
    pub signal_server_ip: String,
    pub signal_server_port: i64,
}

impl Config {
    pub fn from_file(file_path: &str) -> Self {
        let config_str = fs::read_to_string(file_path).expect("Failed to read config file");
        let config: toml::Value = toml::from_str(&config_str).expect("Failed to parse config file");
        Self {
            signal_server_ip: config["signal_server_ip"].as_str().unwrap().to_string(),
            signal_server_port: config["signal_server_port"].as_integer().unwrap(),
        }
    }
}