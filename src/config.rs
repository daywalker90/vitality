use std::{path::Path, sync::Arc};

use anyhow::{anyhow, Error};
use cln_plugin::ConfiguredPlugin;
use log::{info, warn};
use parking_lot::Mutex;
use tokio::fs;

use crate::{structs::Config, PluginState};

pub async fn read_config(
    plugin: &ConfiguredPlugin<PluginState, tokio::io::Stdin, tokio::io::Stdout>,
    state: PluginState,
) -> Result<(), Error> {
    let dir = plugin.configuration().lightning_dir;
    let general_configfile =
        match fs::read_to_string(Path::new(&dir).parent().unwrap().join("config")).await {
            Ok(file2) => file2,
            Err(_) => {
                warn!("No general config file found!");
                String::new()
            }
        };
    let network_configfile = match fs::read_to_string(Path::new(&dir).join("config")).await {
        Ok(file) => file,
        Err(_) => {
            warn!("No network config file found!");
            String::new()
        }
    };

    if general_configfile.is_empty() && network_configfile.is_empty() {
        return Err(anyhow!("No config file found!"));
    }

    parse_config_file(general_configfile, state.config.clone())?;
    parse_config_file(network_configfile, state.config.clone())?;

    Ok(())
}

fn parse_config_file(configfile: String, config: Arc<Mutex<Config>>) -> Result<(), Error> {
    let mut config = config.lock();

    for line in configfile.lines() {
        if line.contains('=') {
            let splitline = line.split('=').collect::<Vec<&str>>();
            if splitline.len() == 2 {
                let name = splitline.clone().into_iter().next().unwrap();
                let value = splitline.into_iter().nth(1).unwrap();

                match name {
                    opt if opt.eq(&config.amboss.0) => match value.parse::<bool>() {
                        Ok(b) => config.amboss.1 = b,
                        Err(e) => {
                            return Err(anyhow!(
                                "Error: Could not parse bool from `{}` for {}: {}",
                                value,
                                config.amboss.0,
                                e
                            ))
                        }
                    },
                    opt if opt.eq(&config.expiring_htlcs.0) => match value.parse::<u32>() {
                        Ok(n) => {
                            if n > 0 {
                                config.expiring_htlcs.1 = n
                            } else {
                                return Err(anyhow!(
                                    "Error: Number needs to be greater than 0 for {}.",
                                    config.expiring_htlcs.0
                                ));
                            }
                        }
                        Err(e) => {
                            return Err(anyhow!(
                                "Error: Could not parse a positive number from `{}` for {}: {}",
                                value,
                                config.expiring_htlcs.0,
                                e
                            ))
                        }
                    },
                    opt if opt.eq(&config.watch_channels.0) => match value.parse::<bool>() {
                        Ok(b) => config.watch_channels.1 = b,
                        Err(e) => {
                            return Err(anyhow!(
                                "Error: Could not parse bool from `{}` for {}: {}",
                                value,
                                config.watch_channels.0,
                                e
                            ))
                        }
                    },
                    opt if opt.eq(&config.watch_gossip.0) => match value.parse::<bool>() {
                        Ok(b) => config.watch_gossip.1 = b,
                        Err(e) => {
                            return Err(anyhow!(
                                "Error: Could not parse bool from `{}` for {}: {}",
                                value,
                                config.watch_gossip.0,
                                e
                            ))
                        }
                    },
                    opt if opt.eq(&config.telegram_token.0) => {
                        config.telegram_token.1 = value.to_string()
                    }
                    opt if opt.eq(&config.telegram_usernames.0) => {
                        config.telegram_usernames.1.push(value.to_string())
                    }
                    opt if opt.eq(&config.smtp_username.0) => {
                        config.smtp_username.1 = value.to_string()
                    }
                    opt if opt.eq(&config.smtp_password.0) => {
                        config.smtp_password.1 = value.to_string()
                    }
                    opt if opt.eq(&config.smtp_server.0) => {
                        config.smtp_server.1 = value.to_string()
                    }
                    opt if opt.eq(&config.smtp_port.0) => match value.parse::<u16>() {
                        Ok(n) => {
                            if n > 0 {
                                config.smtp_port.1 = n
                            } else {
                                return Err(anyhow!(
                                    "Error: Number needs to be greater than 0 for {}.",
                                    config.smtp_port.0
                                ));
                            }
                        }
                        Err(e) => {
                            return Err(anyhow!(
                                "Error: Could not parse a positive number from `{}` for {}: {}",
                                value,
                                config.smtp_port.0,
                                e
                            ))
                        }
                    },
                    opt if opt.eq(&config.email_from.0) => config.email_from.1 = value.to_string(),
                    opt if opt.eq(&config.email_to.0) => config.email_to.1 = value.to_string(),
                    _ => (),
                }
            }
        }
    }

    if !config.telegram_token.1.is_empty() && !config.telegram_usernames.1.is_empty() {
        info!("Will try to notify via telegram");
        config.send_telegram = true;
    } else {
        info!("Insufficient config for telegram notifications. Will not send telegrams.")
    }

    if !config.smtp_username.1.is_empty()
        && !config.smtp_password.1.is_empty()
        && !config.smtp_server.1.is_empty()
        && config.smtp_port.1 > 0
        && !config.email_from.1.is_empty()
        && !config.email_to.1.is_empty()
    {
        info!("Will try to send notifications via email");
        config.send_mail = true;
    } else {
        info!("Insufficient config for email notifications. Will not send emails")
    }

    Ok(())
}
