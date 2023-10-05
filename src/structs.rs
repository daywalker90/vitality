use std::sync::Arc;

use parking_lot::Mutex;

pub const PLUGIN_NAME: &str = "vitality";

#[derive(Clone, Debug)]
pub struct Config {
    pub amboss: (String, bool),
    pub expiring_htlcs: (String, u32),
    pub watch_channels: (String, bool),
    pub watch_gossip: (String, bool),
    pub telegram_token: (String, String),
    pub telegram_usernames: (String, Vec<String>),
    pub smtp_username: (String, String),
    pub smtp_password: (String, String),
    pub smtp_server: (String, String),
    pub smtp_port: (String, u16),
    pub email_from: (String, String),
    pub email_to: (String, String),
    pub send_mail: bool,
    pub send_telegram: bool,
}
impl Config {
    pub fn new() -> Config {
        Config {
            amboss: (PLUGIN_NAME.to_string() + "-amboss", false),
            expiring_htlcs: (PLUGIN_NAME.to_string() + "-expiring-htlcs", 0),
            watch_channels: (PLUGIN_NAME.to_string() + "-watch-channels", true),
            watch_gossip: (PLUGIN_NAME.to_string() + "-watch-gossip", false),
            telegram_token: (PLUGIN_NAME.to_string() + "-telegram-token", "".to_string()),
            telegram_usernames: (PLUGIN_NAME.to_string() + "-telegram-usernames", Vec::new()),
            smtp_username: (PLUGIN_NAME.to_string() + "-smtp-username", "".to_string()),
            smtp_password: (PLUGIN_NAME.to_string() + "-smtp-password", "".to_string()),
            smtp_server: (PLUGIN_NAME.to_string() + "-smtp-server", "".to_string()),
            smtp_port: (PLUGIN_NAME.to_string() + "-smtp-port", 0),
            email_from: (PLUGIN_NAME.to_string() + "-email-from", "".to_string()),
            email_to: (PLUGIN_NAME.to_string() + "-email-to", "".to_string()),
            send_mail: false,
            send_telegram: false,
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
