use std::sync::Arc;

use parking_lot::Mutex;

pub const PLUGIN_NAME: &str = "vitality";

#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct Config {
    pub amboss: bool,
    pub expiring_htlcs: u32,
    pub watch_channels: bool,
    pub watch_gossip: bool,
    pub smtp_username: String,
    pub smtp_password: String,
    pub smtp_server: String,
    pub smtp_port: u16,
    pub email_from: String,
    pub email_to: String,
    pub send_mail: bool,
}
impl Config {
    pub fn new() -> Config {
        Config {
            amboss: false,
            expiring_htlcs: 0,
            watch_channels: true,
            watch_gossip: false,
            smtp_username: String::new(),
            smtp_password: String::new(),
            smtp_server: String::new(),
            smtp_port: 0,
            email_from: String::new(),
            email_to: String::new(),
            send_mail: false,
        }
    }
}

#[derive(Clone)]
pub struct PluginState {
    pub config: Arc<Mutex<Config>>,
}
impl PluginState {
    pub fn new() -> PluginState {
        PluginState {
            config: Arc::new(Mutex::new(Config::new())),
        }
    }
}
