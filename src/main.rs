extern crate serde_json;

use crate::config::read_config;
use crate::util::{send_mail, send_telegram};

use anyhow::anyhow;
use cln_plugin::options::{
    BooleanConfigOption, ConfigOption, IntegerConfigOption, StringConfigOption,
};
use cln_plugin::Builder;

use log::{debug, info, warn};
use rpc::test_notifications;
use structs::{PluginState, PLUGIN_NAME};

mod amboss;
mod channelwatch;
mod config;
mod rpc;
mod structs;
mod util;

const OPT_AMBOSS: BooleanConfigOption =
    ConfigOption::new_bool_no_default("vitality-amboss", "Switch on/off amboss");
const OPT_EXPIRING_HTLCS: IntegerConfigOption = ConfigOption::new_i64_no_default(
    "vitality-expiring-htlcs",
    "Set block amount to watch for expiry",
);
const OPT_WATCH_CHANNELS: BooleanConfigOption =
    ConfigOption::new_bool_no_default("vitality-watch-channels", "Switch on/off watch_channels");
const OPT_WATCH_GOSSIP: BooleanConfigOption =
    ConfigOption::new_bool_no_default("vitality-watch-gossip", "Switch on/off watch_gossip");
const OPT_TELEGRAM_TOKEN: StringConfigOption =
    ConfigOption::new_str_no_default("vitality-telegram-token", "Set telegram token");
const OPT_TELEGRAM_USERNAMES: StringConfigOption =
    ConfigOption::new_str_no_default("vitality-telegram-usernames", "Set telegram users");
const OPT_SMTP_USERNAME: StringConfigOption =
    ConfigOption::new_str_no_default("vitality-smtp-username", "Set smtp username");
const OPT_SMTP_PASSWORD: StringConfigOption =
    ConfigOption::new_str_no_default("vitality-smtp-password", "Set smtp password");
const OPT_SMTP_SERVER: StringConfigOption =
    ConfigOption::new_str_no_default("vitality-smtp-server", "Set smtp server");
const OPT_SMTP_PORT: IntegerConfigOption =
    ConfigOption::new_i64_no_default("vitality-smtp-port", "Set smtp port");
const OPT_EMAIL_FROM: StringConfigOption =
    ConfigOption::new_str_no_default("vitality-email-from", "Set email_from");
const OPT_EMAIL_TO: StringConfigOption =
    ConfigOption::new_str_no_default("vitality-email-to", "Set email_to");

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    std::env::set_var("CLN_PLUGIN_LOG", "cln_plugin=info,cln_rpc=info,debug");
    log_panics::init();
    let state = PluginState::new();
    // let defaultconfig = Config::new();
    let confplugin = match Builder::new(tokio::io::stdin(), tokio::io::stdout())
        .option(OPT_AMBOSS)
        .option(OPT_EXPIRING_HTLCS)
        .option(OPT_WATCH_CHANNELS)
        .option(OPT_WATCH_GOSSIP)
        .option(OPT_TELEGRAM_TOKEN)
        .option(OPT_TELEGRAM_USERNAMES)
        .option(OPT_SMTP_USERNAME)
        .option(OPT_SMTP_PASSWORD)
        .option(OPT_SMTP_SERVER)
        .option(OPT_SMTP_PORT)
        .option(OPT_EMAIL_FROM)
        .option(OPT_EMAIL_TO)
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
            // debug!("read startup options done");
            // match get_startup_options(&plugin, state.clone()) {
            //     Ok(()) => &(),
            //     Err(e) => return plugin.disable(format!("{}", e).as_str()).await,
            // };

            debug!("read config");
            match read_config(&plugin, state.clone()).await {
                Ok(()) => &(),
                Err(e) => return plugin.disable(format!("{}", e).as_str()).await,
            };
            plugin
        }
        None => return Err(anyhow!("Error configuring the plugin!")),
    };
    if let Ok(plugin) = confplugin.start(state).await {
        let config = plugin.state().config.lock().clone();

        if config.amboss.1 {
            info!("Starting amboss online ping task");
            let healthclone = plugin.clone();
            tokio::spawn(async move {
                match amboss::amboss_ping_loop(healthclone.clone()).await {
                    Ok(()) => (),
                    Err(e) => {
                        warn!("Error in amboss_ping_loop thread: {}", e.to_string());
                        let config = healthclone.state().config.lock().clone();
                        let subject = "ALARM: amboss_ping_loop Error".to_string();
                        let body = e.to_string();
                        if config.send_mail {
                            match send_mail(&config, &subject, &body, false).await {
                                Ok(_) => (),
                                Err(er) => {
                                    warn!("amboss_ping_loop: Mail failed: {}", er.to_string())
                                }
                            };
                        }
                        if config.send_telegram {
                            match send_telegram(&config, &subject, &body).await {
                                Ok(_) => (),
                                Err(er) => {
                                    warn!("amboss_ping_loop: Telegram failed: {}", er.to_string())
                                }
                            };
                        }
                    }
                };
            });
        }

        if config.expiring_htlcs.1 > 0 || config.watch_channels.1 {
            let channel_clone = plugin.clone();
            tokio::spawn(async move {
                match channelwatch::check_channels_loop(channel_clone.clone()).await {
                    Ok(()) => (),
                    Err(e) => {
                        warn!("Error in check_channels_loop thread: {}", e.to_string());
                        let config = channel_clone.state().config.lock().clone();
                        let subject = "ALARM: check_channels_loop Error".to_string();
                        let body = e.to_string();
                        if config.send_mail {
                            match send_mail(&config, &subject, &body, false).await {
                                Ok(_) => (),
                                Err(er) => warn!(
                                    "check_channels_loop: Unexpected Error: {}",
                                    er.to_string()
                                ),
                            };
                        }
                        if config.send_telegram {
                            match send_telegram(&config, &subject, &body).await {
                                Ok(_) => (),
                                Err(er) => warn!(
                                    "check_channels_loop: Unexpected Error: {}",
                                    er.to_string()
                                ),
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
