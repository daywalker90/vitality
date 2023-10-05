extern crate serde_json;

use crate::config::read_config;
use crate::util::{send_mail, send_telegram};

use anyhow::anyhow;
use cln_plugin::{options, Builder};

use log::{debug, info, warn};
use rpc::test_notifications;
use structs::{PluginState, PLUGIN_NAME};

mod amboss;
mod channelwatch;
mod config;
mod rpc;
mod structs;
mod util;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    std::env::set_var("CLN_PLUGIN_LOG", "trace");
    log_panics::init();
    let state = PluginState::new();
    // let defaultconfig = Config::new();
    let confplugin = match Builder::new(tokio::io::stdin(), tokio::io::stdout())
        .option(options::ConfigOption::new(
            &(PLUGIN_NAME.to_string() + "-amboss"),
            options::Value::OptBoolean,
            "Switch on/off amboss",
        ))
        .option(options::ConfigOption::new(
            &(PLUGIN_NAME.to_string() + "-expiring-htlcs"),
            options::Value::OptInteger,
            "Set block amount to watch for expiry",
        ))
        .option(options::ConfigOption::new(
            &(PLUGIN_NAME.to_string() + "-watch-channels"),
            options::Value::OptBoolean,
            "Switch on/off watch_channels",
        ))
        .option(options::ConfigOption::new(
            &(PLUGIN_NAME.to_string() + "-watch-gossip"),
            options::Value::OptBoolean,
            "Switch on/off watch_gossip",
        ))
        .option(options::ConfigOption::new(
            &(PLUGIN_NAME.to_string() + "-telegram-token"),
            options::Value::OptString,
            "Set telegram token",
        ))
        .option(options::ConfigOption::new(
            &(PLUGIN_NAME.to_string() + "-telegram-usernames"),
            options::Value::OptString,
            "Set telegram users",
        ))
        .option(options::ConfigOption::new(
            &(PLUGIN_NAME.to_string() + "-smtp-username"),
            options::Value::OptString,
            "Set smtp username",
        ))
        .option(options::ConfigOption::new(
            &(PLUGIN_NAME.to_string() + "-smtp-password"),
            options::Value::OptString,
            "Set smtp password",
        ))
        .option(options::ConfigOption::new(
            &(PLUGIN_NAME.to_string() + "-smtp-server"),
            options::Value::OptString,
            "Set smtp server",
        ))
        .option(options::ConfigOption::new(
            &(PLUGIN_NAME.to_string() + "-smtp-port"),
            options::Value::OptInteger,
            "Set smtp port",
        ))
        .option(options::ConfigOption::new(
            &(PLUGIN_NAME.to_string() + "-email-from"),
            options::Value::OptString,
            "Set email from",
        ))
        .option(options::ConfigOption::new(
            &(PLUGIN_NAME.to_string() + "-email-to"),
            options::Value::OptString,
            "Set email to",
        ))
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
