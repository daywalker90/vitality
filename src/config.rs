use anyhow::{anyhow, Error};
use cln_plugin::{options, ConfiguredPlugin, Plugin};
use cln_rpc::{model::responses::GetinfoResponse, RpcError};
use log::info;
use serde_json::json;

use crate::{
    structs::Config, util::at_or_above_version, PluginState, OPT_AMBOSS, OPT_EMAIL_FROM,
    OPT_EMAIL_TO, OPT_EXPIRING_HTLCS, OPT_SMTP_PASSWORD, OPT_SMTP_PORT, OPT_SMTP_SERVER,
    OPT_SMTP_USERNAME, OPT_TELEGRAM_TOKEN, OPT_TELEGRAM_USERNAMES, OPT_WATCH_CHANNELS,
    OPT_WATCH_GOSSIP,
};

pub async fn setconfig_callback(
    plugin: Plugin<PluginState>,
    args: serde_json::Value,
) -> Result<serde_json::Value, Error> {
    let name = args
        .get("config")
        .ok_or_else(|| anyhow!("Bad CLN object. No option name found!"))?
        .as_str()
        .ok_or_else(|| anyhow!("Bad CLN object. Option name not a string!"))?;
    let value = args
        .get("val")
        .ok_or_else(|| anyhow!("Bad CLN object. No value found for option: {name}"))?;

    let opt_value = parse_option(name, value).map_err(|e| {
        anyhow!(json!(RpcError {
            code: Some(-32602),
            message: e.to_string(),
            data: None
        }))
    })?;

    let mut config = plugin.state().config.lock();

    check_option(&mut config, name, &opt_value).map_err(|e| {
        anyhow!(json!(RpcError {
            code: Some(-32602),
            message: e.to_string(),
            data: None
        }))
    })?;

    plugin.set_option_str(name, opt_value).map_err(|e| {
        anyhow!(json!(RpcError {
            code: Some(-32602),
            message: e.to_string(),
            data: None
        }))
    })?;

    activate_mail(&mut config);
    activate_telegram(&mut config);

    Ok(json!({}))
}

fn parse_option(name: &str, value: &serde_json::Value) -> Result<options::Value, Error> {
    match name {
        n if n.eq(OPT_EXPIRING_HTLCS) || n.eq(OPT_SMTP_PORT) => {
            if let Some(n_i64) = value.as_i64() {
                return Ok(options::Value::Integer(n_i64));
            } else if let Some(n_str) = value.as_str() {
                if let Ok(n_neg_i64) = n_str.parse::<i64>() {
                    return Ok(options::Value::Integer(n_neg_i64));
                }
            }
            Err(anyhow!("{} is not a valid integer!", name))
        }
        n if n.eq(OPT_AMBOSS) || n.eq(OPT_WATCH_CHANNELS) || n.eq(OPT_WATCH_GOSSIP) => {
            if let Some(n_bool) = value.as_bool() {
                return Ok(options::Value::Boolean(n_bool));
            } else if let Some(n_str) = value.as_str() {
                if let Ok(n_str_bool) = n_str.parse::<bool>() {
                    return Ok(options::Value::Boolean(n_str_bool));
                }
            }
            Err(anyhow!("{} is not a valid boolean!", n))
        }
        _ => {
            if value.is_string() {
                Ok(options::Value::String(value.as_str().unwrap().to_owned()))
            } else {
                Err(anyhow!("{} is not a valid string!", name))
            }
        }
    }
}

pub async fn get_startup_options(
    plugin: &ConfiguredPlugin<PluginState, tokio::io::Stdin, tokio::io::Stdout>,
    state: PluginState,
    info: GetinfoResponse,
) -> Result<(), Error> {
    let mut config = state.config.lock();

    config.is_at_or_above_24_11 = at_or_above_version(&info.version, "24.11")?;

    if let Some(utf8) = plugin.option_str(OPT_AMBOSS)? {
        check_option(&mut config, OPT_AMBOSS, &utf8)?;
    };
    if let Some(exp) = plugin.option_str(OPT_EXPIRING_HTLCS)? {
        check_option(&mut config, OPT_EXPIRING_HTLCS, &exp)?;
    };
    if let Some(watch) = plugin.option_str(OPT_WATCH_CHANNELS)? {
        check_option(&mut config, OPT_WATCH_CHANNELS, &watch)?;
    };
    if let Some(goss) = plugin.option_str(OPT_WATCH_GOSSIP)? {
        check_option(&mut config, OPT_WATCH_GOSSIP, &goss)?;
    };
    if let Some(tok) = plugin.option_str(OPT_TELEGRAM_TOKEN)? {
        check_option(&mut config, OPT_TELEGRAM_TOKEN, &tok)?;
    };
    if let Some(tusers) = plugin.option_str(OPT_TELEGRAM_USERNAMES)? {
        check_option(&mut config, OPT_TELEGRAM_USERNAMES, &tusers)?;
    };
    if let Some(smtpuser) = plugin.option_str(OPT_SMTP_USERNAME)? {
        check_option(&mut config, OPT_SMTP_USERNAME, &smtpuser)?;
    };
    if let Some(smtppw) = plugin.option_str(OPT_SMTP_PASSWORD)? {
        check_option(&mut config, OPT_SMTP_PASSWORD, &smtppw)?;
    };
    if let Some(smtpserver) = plugin.option_str(OPT_SMTP_SERVER)? {
        check_option(&mut config, OPT_SMTP_SERVER, &smtpserver)?;
    };
    if let Some(smtpport) = plugin.option_str(OPT_SMTP_PORT)? {
        check_option(&mut config, OPT_SMTP_PORT, &smtpport)?;
    };
    if let Some(emailfrom) = plugin.option_str(OPT_EMAIL_FROM)? {
        check_option(&mut config, OPT_EMAIL_FROM, &emailfrom)?;
    };
    if let Some(emailto) = plugin.option_str(OPT_EMAIL_TO)? {
        check_option(&mut config, OPT_EMAIL_TO, &emailto)?;
    };

    activate_mail(&mut config);
    activate_telegram(&mut config);

    Ok(())
}

fn activate_mail(config: &mut Config) {
    if !config.smtp_username.is_empty()
        && !config.smtp_password.is_empty()
        && !config.smtp_server.is_empty()
        && config.smtp_port > 0
        && !config.email_from.is_empty()
        && !config.email_to.is_empty()
    {
        info!("Will try to send notifications via email");
        config.send_mail = true;
    } else {
        info!("Insufficient config for email notifications. Will not send emails")
    }
}

fn activate_telegram(config: &mut Config) {
    if !config.telegram_token.is_empty() && !config.telegram_usernames.is_empty() {
        info!(
            "Will try to notify {} via telegram",
            config.telegram_usernames.join(", ")
        );
        config.send_telegram = true;
    } else {
        info!("Insufficient config for telegram notifications. Will not send telegrams.")
    }
}

fn check_option(config: &mut Config, name: &str, value: &options::Value) -> Result<(), Error> {
    match name {
        n if n.eq(OPT_AMBOSS) => config.amboss = value.as_bool().unwrap(),
        n if n.eq(OPT_EXPIRING_HTLCS) => {
            config.expiring_htlcs = u32::try_from(value.as_i64().unwrap())?
        }
        n if n.eq(OPT_WATCH_CHANNELS) => config.watch_channels = value.as_bool().unwrap(),
        n if n.eq(OPT_WATCH_GOSSIP) => config.watch_gossip = value.as_bool().unwrap(),
        n if n.eq(OPT_TELEGRAM_TOKEN) => {
            config.telegram_token = value.as_str().unwrap().to_string()
        }
        n if n.eq(OPT_TELEGRAM_USERNAMES) => {
            let users = value.as_str().unwrap().split(',').collect::<Vec<&str>>();
            for user in users {
                config.telegram_usernames.push(user.trim().to_string())
            }
        }
        n if n.eq(OPT_SMTP_USERNAME) => config.smtp_username = value.as_str().unwrap().to_string(),
        n if n.eq(OPT_SMTP_PASSWORD) => config.smtp_password = value.as_str().unwrap().to_string(),
        n if n.eq(OPT_SMTP_SERVER) => config.smtp_server = value.as_str().unwrap().to_string(),
        n if n.eq(OPT_SMTP_PORT) => config.smtp_port = u16::try_from(value.as_i64().unwrap())?,
        n if n.eq(OPT_EMAIL_FROM) => config.email_from = value.as_str().unwrap().to_string(),
        n if n.eq(OPT_EMAIL_TO) => config.email_to = value.as_str().unwrap().to_string(),
        _ => return Err(anyhow!("Unknown option: {}", name)),
    }
    Ok(())
}
