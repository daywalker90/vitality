use std::sync::Arc;

use parking_lot::Mutex;

use crate::{
    OPT_AMBOSS, OPT_EMAIL_FROM, OPT_EMAIL_TO, OPT_EXPIRING_HTLCS, OPT_SMTP_PASSWORD, OPT_SMTP_PORT,
    OPT_SMTP_SERVER, OPT_SMTP_USERNAME, OPT_TELEGRAM_TOKEN, OPT_TELEGRAM_USERNAMES,
    OPT_WATCH_CHANNELS, OPT_WATCH_GOSSIP,
};

pub const PLUGIN_NAME: &str = "vitality";

#[derive(Clone, Debug)]
pub struct Config {
    pub amboss: DynamicConfigOption<bool>,
    pub expiring_htlcs: DynamicConfigOption<u32>,
    pub watch_channels: DynamicConfigOption<bool>,
    pub watch_gossip: DynamicConfigOption<bool>,
    pub telegram_token: DynamicConfigOption<String>,
    pub telegram_usernames: DynamicConfigOption<Vec<String>>,
    pub smtp_username: DynamicConfigOption<String>,
    pub smtp_password: DynamicConfigOption<String>,
    pub smtp_server: DynamicConfigOption<String>,
    pub smtp_port: DynamicConfigOption<u16>,
    pub email_from: DynamicConfigOption<String>,
    pub email_to: DynamicConfigOption<String>,
    pub send_mail: bool,
    pub send_telegram: bool,
}
impl Config {
    pub fn new() -> Config {
        Config {
            amboss: DynamicConfigOption {
                name: OPT_AMBOSS,
                value: false,
            },
            expiring_htlcs: DynamicConfigOption {
                name: OPT_EXPIRING_HTLCS,
                value: 0,
            },
            watch_channels: DynamicConfigOption {
                name: OPT_WATCH_CHANNELS,
                value: true,
            },
            watch_gossip: DynamicConfigOption {
                name: OPT_WATCH_GOSSIP,
                value: false,
            },
            telegram_token: DynamicConfigOption {
                name: OPT_TELEGRAM_TOKEN,
                value: String::new(),
            },
            telegram_usernames: DynamicConfigOption {
                name: OPT_TELEGRAM_USERNAMES,
                value: Vec::new(),
            },
            smtp_username: DynamicConfigOption {
                name: OPT_SMTP_USERNAME,
                value: String::new(),
            },
            smtp_password: DynamicConfigOption {
                name: OPT_SMTP_PASSWORD,
                value: String::new(),
            },
            smtp_server: DynamicConfigOption {
                name: OPT_SMTP_SERVER,
                value: String::new(),
            },
            smtp_port: DynamicConfigOption {
                name: OPT_SMTP_PORT,
                value: 0,
            },
            email_from: DynamicConfigOption {
                name: OPT_EMAIL_FROM,
                value: String::new(),
            },
            email_to: DynamicConfigOption {
                name: OPT_EMAIL_TO,
                value: String::new(),
            },
            send_mail: false,
            send_telegram: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DynamicConfigOption<T> {
    pub name: &'static str,
    pub value: T,
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
