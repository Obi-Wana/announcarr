use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub announced_file: String,
}

#[derive(Debug, Deserialize)]
pub struct ApiConfig {
    pub url: String,
    pub token: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct IrcConfig {
    pub server: String,
    pub port: u16,
    pub use_tls: bool,
    pub channel: String,
    pub nickname: String,
    pub password: String,
    pub oper: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub app: AppConfig,
    pub api: ApiConfig,
    pub irc: IrcConfig,
}

pub fn load_config() -> Config {
    let config_str = fs::read_to_string("config.toml").expect("Failed to read config file");
    toml::from_str(&config_str).expect("Failed to parse config")
}
