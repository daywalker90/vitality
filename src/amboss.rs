use std::time::Duration;

use anyhow::{Error, anyhow};
use chrono::Utc;
use cln_plugin::Plugin;
use cln_rpc::{ClnRpc, model::requests::SignmessageRequest};
use log::{info, warn};
use serde_json::{Value, json};
use tokio::time::{self, Instant};

use crate::{
    structs::PluginState,
    util::{make_rpc_path, send_mail},
};

async fn amboss_ping(plugin: Plugin<PluginState>) -> Result<(), Error> {
    let now = Instant::now();
    info!("Creating amboss ping");

    let rpc_path = make_rpc_path(&plugin);
    let mut rpc = ClnRpc::new(&rpc_path).await?;

    let url = "https://api.amboss.space/graphql";
    let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%S%z").to_string();
    info!("Timestamp: {timestamp}");

    let signature = rpc
        .call_typed(&SignmessageRequest {
            message: timestamp.clone(),
        })
        .await?;
    info!("Signature: {}", signature.zbase);

    let variables = json!({
        "signature": signature.zbase,
        "timestamp": timestamp,
    });

    let json_data = json!({
        "query": "mutation HealthCheck($signature: String!, $timestamp: String!) \
        { healthCheck(signature: $signature, timestamp: $timestamp) }",
        "variables": variables,
    });

    info!("Sending ping...");
    let response = bitreq::post(url)
        .with_header("Content-Type", "application/json")
        .with_body(json_data.to_string())
        .send_async()
        .await?;

    let response_text = response.as_str()?;
    let json: Value = serde_json::from_str(response_text)?;
    let mut health_check_success = false;
    if let Some(data) = json.get("data") {
        if let Some(health_check) = data.get("healthCheck") {
            if let Some(health_check_value) = health_check.as_bool() {
                if health_check_value {
                    health_check_success = true;
                }
            }
        }
    }
    if health_check_success {
        info!("Amboss ping succeeded in: {}ms", now.elapsed().as_millis());
        Ok(())
    } else {
        Err(anyhow!("Amboss ping error: {response_text}"))
    }
}

pub async fn amboss_ping_loop(plugin: Plugin<PluginState>) -> Result<(), Error> {
    let mut sleep_time_s = 300;
    loop {
        {
            match amboss_ping(plugin.clone()).await {
                Ok(_succ) => sleep_time_s = 300,
                Err(e) => {
                    warn!("Error in amboss_ping: {e}");

                    if sleep_time_s >= 300 {
                        sleep_time_s = 10;
                    } else {
                        let config = plugin.state().config.lock().clone();
                        let subject = "Amboss error".to_string();
                        let body = e.to_string();
                        if config.send_mail {
                            if let Err(e) = send_mail(&config, subject, body, false).await {
                                warn!("amboss_ping_loop: Error sending mail: {e}");
                            }
                        }
                        sleep_time_s += 10;
                    }
                }
            }
        }
        time::sleep(Duration::from_secs(sleep_time_s)).await;
    }
}
