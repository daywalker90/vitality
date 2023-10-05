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
        .from(config.email_from.1.parse().unwrap())
        .to(config.email_to.1.parse().unwrap())
        .subject(subject.clone())
        .header(header)
        .body(body.to_string())
        .unwrap();

    let creds = Credentials::new(
        config.smtp_username.1.clone(),
        config.smtp_password.1.clone(),
    );

    let tls_parameters = TlsParameters::builder(config.smtp_server.1.clone())
        .dangerous_accept_invalid_certs(false)
        .dangerous_accept_invalid_hostnames(false)
        .build_native()?;

    let mailer = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.smtp_server.1)?
        .credentials(creds)
        .tls(Tls::Required(tls_parameters))
        .port(config.smtp_port.1)
        .timeout(Some(Duration::from_secs(60)))
        .build();

    // Send the email
    let result = mailer.send(email).await;
    if result.is_ok() {
        info!(
            "Sent email with subject: `{}` to: `{}`",
            subject, config.email_to.1
        );
        Ok(())
    } else {
        Err(anyhow!("Failed to send email: {:?}", result))
    }
}

pub async fn send_telegram(config: &Config, subject: &String, body: &String) -> Result<(), Error> {
    let bot = Bot::new(config.telegram_token.1.clone());

    for username in &config.telegram_usernames.1 {
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

pub async fn get_config_path(lightning_dir: String) -> Result<Vec<String>, Error> {
    let lightning_dir_network = Path::new(&lightning_dir);
    let lightning_dir_general = Path::new(&lightning_dir).parent().unwrap();
    Ok(vec![
        lightning_dir_general
            .join("config")
            .to_str()
            .unwrap()
            .to_string(),
        lightning_dir_network
            .join("config")
            .to_str()
            .unwrap()
            .to_string(),
    ])
}
