use std::path::Path;

use cln_rpc::model::requests::GetinfoRequest;
use cln_rpc::ClnRpc;
use config::setconfig_callback;
use mimalloc::MiMalloc;
use serde_json::json;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

extern crate serde_json;

use crate::config::get_startup_options;
use crate::util::{send_mail, send_telegram};

use anyhow::anyhow;
use cln_plugin::options::{
    BooleanConfigOption, ConfigOption, IntegerConfigOption, StringConfigOption,
};
use cln_plugin::{Builder, Error, Plugin};

use log::{info, warn};
use structs::{PluginState, PLUGIN_NAME};

mod amboss;
mod channelwatch;
mod config;
mod structs;
mod util;

const OPT_AMBOSS: &str = "vitality-amboss";
const OPT_EXPIRING_HTLCS: &str = "vitality-expiring-htlcs";
const OPT_WATCH_CHANNELS: &str = "vitality-watch-channels";
const OPT_WATCH_GOSSIP: &str = "vitality-watch-gossip";
const OPT_TELEGRAM_TOKEN: &str = "vitality-telegram-token";
const OPT_TELEGRAM_USERNAMES: &str = "vitality-telegram-usernames";
const OPT_SMTP_USERNAME: &str = "vitality-smtp-username";
const OPT_SMTP_PASSWORD: &str = "vitality-smtp-password";
const OPT_SMTP_SERVER: &str = "vitality-smtp-server";
const OPT_SMTP_PORT: &str = "vitality-smtp-port";
const OPT_EMAIL_FROM: &str = "vitality-email-from";
const OPT_EMAIL_TO: &str = "vitality-email-to";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    std::env::set_var("CLN_PLUGIN_LOG", "vitality=debug,info");
    log_panics::init();
    let state = PluginState::new();
    let opt_amboss: BooleanConfigOption =
        ConfigOption::new_bool_no_default(OPT_AMBOSS, "Switch on/off amboss").dynamic();
    let opt_expiring_htlcs: IntegerConfigOption = ConfigOption::new_i64_no_default(
        OPT_EXPIRING_HTLCS,
        "Set block amount to watch for expiry",
    )
    .dynamic();
    let opt_watch_channels: BooleanConfigOption =
        ConfigOption::new_bool_no_default(OPT_WATCH_CHANNELS, "Switch on/off watch_channels")
            .dynamic();
    let opt_watch_gossip: BooleanConfigOption =
        ConfigOption::new_bool_no_default(OPT_WATCH_GOSSIP, "Switch on/off watch_gossip").dynamic();
    let opt_telegram_token: StringConfigOption =
        ConfigOption::new_str_no_default(OPT_TELEGRAM_TOKEN, "Set telegram token").dynamic();
    let opt_telegram_usernames: StringConfigOption =
        ConfigOption::new_str_no_default(OPT_TELEGRAM_USERNAMES, "Set telegram users").dynamic();
    let opt_smtp_username: StringConfigOption =
        ConfigOption::new_str_no_default(OPT_SMTP_USERNAME, "Set smtp username").dynamic();
    let opt_smtp_password: StringConfigOption =
        ConfigOption::new_str_no_default(OPT_SMTP_PASSWORD, "Set smtp password").dynamic();
    let opt_smtp_server: StringConfigOption =
        ConfigOption::new_str_no_default(OPT_SMTP_SERVER, "Set smtp server").dynamic();
    let opt_smtp_port: IntegerConfigOption =
        ConfigOption::new_i64_no_default(OPT_SMTP_PORT, "Set smtp port").dynamic();
    let opt_email_from: StringConfigOption =
        ConfigOption::new_str_no_default(OPT_EMAIL_FROM, "Set email_from").dynamic();
    let opt_email_to: StringConfigOption =
        ConfigOption::new_str_no_default(OPT_EMAIL_TO, "Set email_to");

    let confplugin = match Builder::new(tokio::io::stdin(), tokio::io::stdout())
        .option(opt_amboss)
        .option(opt_expiring_htlcs)
        .option(opt_watch_channels)
        .option(opt_watch_gossip)
        .option(opt_telegram_token)
        .option(opt_telegram_usernames)
        .option(opt_smtp_username)
        .option(opt_smtp_password)
        .option(opt_smtp_server)
        .option(opt_smtp_port)
        .option(opt_email_from)
        .option(opt_email_to)
        .setconfig_callback(setconfig_callback)
        .rpcmethod(
            &(PLUGIN_NAME.to_string() + "-testnotifications"),
            "test notifications settings",
            test_notifications,
        )
        .dynamic()
        .configure()
        .await?
    {
        Some(plugin) => {
            // debug!("read config");
            // match read_config(&plugin, state.clone()).await {
            //     Ok(()) => &(),
            //     Err(e) => return plugin.disable(format!("{}", e).as_str()).await,
            // };
            let rpc_path = Path::new(&plugin.configuration().lightning_dir)
                .join(plugin.configuration().rpc_file);
            let mut rpc = ClnRpc::new(&rpc_path).await?;
            let getinfo = rpc.call_typed(&GetinfoRequest {}).await?;
            match get_startup_options(&plugin, state.clone(), getinfo).await {
                Ok(()) => &(),
                Err(e) => return plugin.disable(format!("{}", e).as_str()).await,
            };
            info!("read startup options");
            plugin
        }
        None => return Err(anyhow!("Error configuring the plugin!")),
    };
    if let Ok(plugin) = confplugin.start(state).await {
        let config = plugin.state().config.lock().clone();

        if config.amboss {
            info!("Starting amboss online ping task");
            let healthclone = plugin.clone();
            tokio::spawn(async move {
                match amboss::amboss_ping_loop(healthclone.clone()).await {
                    Ok(()) => (),
                    Err(e) => {
                        warn!("Error in amboss_ping_loop thread: {}", e);
                        let config = healthclone.state().config.lock().clone();
                        let subject = "ALARM: amboss_ping_loop Error".to_string();
                        let body = e.to_string();
                        if config.send_mail {
                            match send_mail(&config, &subject, &body, false).await {
                                Ok(_) => (),
                                Err(er) => {
                                    warn!("amboss_ping_loop: Mail failed: {}", er)
                                }
                            };
                        }
                        if config.send_telegram {
                            match send_telegram(&config, &subject, &body).await {
                                Ok(_) => (),
                                Err(er) => {
                                    warn!("amboss_ping_loop: Telegram failed: {}", er)
                                }
                            };
                        }
                    }
                };
            });
        }

        if config.expiring_htlcs > 0 || config.watch_channels {
            let channel_clone = plugin.clone();
            tokio::spawn(async move {
                match channelwatch::check_channels_loop(channel_clone.clone()).await {
                    Ok(()) => (),
                    Err(e) => {
                        warn!("Error in check_channels_loop thread: {}", e);
                        let config = channel_clone.state().config.lock().clone();
                        let subject = "ALARM: check_channels_loop Error".to_string();
                        let body = e.to_string();
                        if config.send_mail {
                            match send_mail(&config, &subject, &body, false).await {
                                Ok(_) => (),
                                Err(er) => warn!("check_channels_loop: Unexpected Error: {}", er),
                            };
                        }
                        if config.send_telegram {
                            match send_telegram(&config, &subject, &body).await {
                                Ok(_) => (),
                                Err(er) => warn!("check_channels_loop: Unexpected Error: {}", er),
                            };
                        }
                    }
                };
            });
        }

        plugin.join().await
    } else {
        Err(anyhow!("Error starting the plugin!"))
    }
}

async fn test_notifications(
    plugin: Plugin<PluginState>,
    _args: serde_json::Value,
) -> Result<serde_json::Value, Error> {
    let config = plugin.state().config.lock().clone();
    let subject = "Test Notification".to_string();
    let body = "This is a test notification sent from vitality".to_string();
    if config.send_mail {
        send_mail(&config, &subject, &body, false).await?;
    }
    if config.send_telegram {
        send_telegram(&config, &subject, &body).await?;
    }
    Ok(json!({"format-hint":"simple","result":"success"}))
}
