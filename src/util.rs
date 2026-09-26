use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Error, anyhow};
use cln_plugin::Plugin;
use lettre::{
    AsyncSmtpTransport,
    AsyncTransport,
    Message,
    Tokio1Executor,
    message::header::ContentType,
    transport::smtp::{
        authentication::Credentials,
        client::{Tls, TlsParameters},
    },
};
use log::info;

use crate::structs::{Config, PluginState};

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
    subject: String,
    body: String,
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
        .body(body)
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
        Err(anyhow!("Failed to send email: {result:?}"))
    }
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
