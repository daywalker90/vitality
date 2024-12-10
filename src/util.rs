use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{anyhow, Error};
use cln_plugin::Plugin;
use log::{info, warn};
use teloxide::{requests::Requester, Bot};

use crate::structs::{Config, PluginState};
use lettre::{
    message::header::ContentType,
    transport::smtp::{
        authentication::Credentials,
        client::{Tls, TlsParameters},
    },
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};

// pub async fn get_alias_map(
//     plugin: Plugin<PluginState>,
// ) -> Result<BTreeMap<PublicKey, String>, Error> {
//     let rpc_path = make_rpc_path(&plugin);

//     let now = Instant::now();
//     let nodes = list_nodes(&rpc_path, None).await?.nodes;
//     let alias_map = nodes
//         .into_iter()
//         .filter_map(|node| {
//             node.alias
//                 .map(|alias| (node.nodeid, alias.replace(|c: char| !c.is_ascii(), "?")))
//         })
//         .collect::<BTreeMap<PublicKey, String>>();
//     info!(
//         "Refreshing alias map done in {}ms!",
//         now.elapsed().as_millis().to_string()
//     );
//     Ok(alias_map)
// }

pub async fn send_mail(
    config: &Config,
    subject: &String,
    body: &String,
    html: bool,
) -> Result<(), Error> {
    let header = if html {
        ContentType::TEXT_HTML
    } else {
        ContentType::TEXT_PLAIN
    };

    let email = Message::builder()
        .from(config.email_from.parse().unwrap())
        .to(config.email_to.parse().unwrap())
        .subject(subject.clone())
        .header(header)
        .body(body.to_string())
        .unwrap();

    let creds = Credentials::new(config.smtp_username.clone(), config.smtp_password.clone());

    let tls_parameters = TlsParameters::builder(config.smtp_server.clone())
        .dangerous_accept_invalid_certs(false)
        .build_rustls()?;

    let mailer = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.smtp_server)?
        .credentials(creds)
        .tls(Tls::Required(tls_parameters))
        .port(config.smtp_port)
        .timeout(Some(Duration::from_secs(60)))
        .build();

    // Send the email
    let result = mailer.send(email).await;
    if result.is_ok() {
        info!(
            "Sent email with subject: `{}` to: `{}`",
            subject, config.email_to
        );
        Ok(())
    } else {
        Err(anyhow!("Failed to send email: {:?}", result))
    }
}

pub async fn send_telegram(config: &Config, subject: &String, body: &String) -> Result<(), Error> {
    let bot = Bot::new(config.telegram_token.clone());

    for username in &config.telegram_usernames {
        let mut message = format!("{}\n{}", subject, body);
        if message.len() > 4000 {
            message = message[..4000].to_string()
        }
        if let Err(e) = bot.send_message(username.clone(), message).await {
            warn!("Error sending telegram to {}: {}", username, e);
        };
    }
    Ok(())
}

pub fn make_rpc_path(plugin: &Plugin<PluginState>) -> PathBuf {
    Path::new(&plugin.configuration().lightning_dir).join(plugin.configuration().rpc_file)
}

pub fn parse_boolean(s: &str) -> Option<bool> {
    match s.to_lowercase().as_str() {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

pub fn at_or_above_version(my_version: &str, min_version: &str) -> Result<bool, Error> {
    let clean_start_my_version = my_version.trim_start_matches('v');
    let full_clean_my_version: String = clean_start_my_version
        .chars()
        .take_while(|x| x.is_ascii_digit() || *x == '.')
        .collect();

    let my_version_parts: Vec<&str> = full_clean_my_version.split('.').collect();
    let min_version_parts: Vec<&str> = min_version.split('.').collect();

    if my_version_parts.len() <= 1 || my_version_parts.len() > 3 {
        return Err(anyhow!("Version string parse error: {}", my_version));
    }
    for (my, min) in my_version_parts.iter().zip(min_version_parts.iter()) {
        let my_num: u32 = my.parse()?;
        let min_num: u32 = min.parse()?;

        if my_num != min_num {
            return Ok(my_num > min_num);
        }
    }

    Ok(my_version_parts.len() >= min_version_parts.len())
}
