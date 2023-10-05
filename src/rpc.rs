use std::path::PathBuf;

use anyhow::{anyhow, Error};
use cln_plugin::Plugin;
use cln_rpc::{
    model::requests::{
        ConnectRequest, DisconnectRequest, GetinfoRequest, ListchannelsRequest,
        ListpeerchannelsRequest, SignmessageRequest,
    },
    model::responses::{
        ConnectResponse, DisconnectResponse, GetinfoResponse, ListchannelsResponse,
        ListpeerchannelsResponse, SignmessageResponse,
    },
    primitives::{PublicKey, ShortChannelId},
    ClnRpc, Request, Response,
};
use serde_json::json;

use crate::{
    structs::PluginState,
    util::{send_mail, send_telegram},
};

pub async fn test_notifications(
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

pub async fn signmessage(
    rpc_path: &PathBuf,
    message: String,
) -> Result<SignmessageResponse, Error> {
    let mut rpc = ClnRpc::new(&rpc_path).await?;
    let signmessage_request = rpc
        .call(Request::SignMessage(SignmessageRequest { message }))
        .await
        .map_err(|e| anyhow!("Error calling signmessage: {}", e.to_string()))?;
    match signmessage_request {
        Response::SignMessage(info) => Ok(info),
        e => Err(anyhow!("Unexpected result in signmessage: {:?}", e)),
    }
}

pub async fn list_peer_channels(
    rpc_path: &PathBuf,
    id: Option<PublicKey>,
) -> Result<ListpeerchannelsResponse, Error> {
    let mut rpc = ClnRpc::new(&rpc_path).await?;
    let list_peer_channels = rpc
        .call(Request::ListPeerChannels(ListpeerchannelsRequest { id }))
        .await
        .map_err(|e| anyhow!("Error calling list_peer_channels: {}", e.to_string()))?;
    match list_peer_channels {
        Response::ListPeerChannels(info) => Ok(info),
        e => Err(anyhow!("Unexpected result in list_peer_channels: {:?}", e)),
    }
}

pub async fn connect(
    rpc_path: &PathBuf,
    id: PublicKey,
    host: Option<String>,
    port: Option<u16>,
) -> Result<ConnectResponse, Error> {
    let mut rpc = ClnRpc::new(&rpc_path).await?;
    let connect_request = rpc
        .call(Request::Connect(ConnectRequest {
            id: id.to_string(),
            host,
            port,
        }))
        .await?;
    match connect_request {
        Response::Connect(info) => Ok(info),
        e => Err(anyhow!("Unexpected result in connect: {:?}", e)),
    }
}

pub async fn disconnect(rpc_path: &PathBuf, id: PublicKey) -> Result<DisconnectResponse, Error> {
    let mut rpc = ClnRpc::new(&rpc_path).await?;
    let disconnect_request = rpc
        .call(Request::Disconnect(DisconnectRequest {
            id,
            force: Some(true),
        }))
        .await?;
    match disconnect_request {
        Response::Disconnect(info) => Ok(info),
        e => Err(anyhow!("Unexpected result in disconnect: {:?}", e)),
    }
}

pub async fn get_info(rpc_path: &PathBuf) -> Result<GetinfoResponse, Error> {
    let mut rpc = ClnRpc::new(&rpc_path).await?;
    let getinfo_request = rpc
        .call(Request::Getinfo(GetinfoRequest {}))
        .await
        .map_err(|e| anyhow!("Error calling get_info: {}", e.to_string()))?;
    match getinfo_request {
        Response::Getinfo(info) => Ok(info),
        e => Err(anyhow!("Unexpected result in get_info: {:?}", e)),
    }
}

pub async fn list_channels(
    rpc_path: &PathBuf,
    short_channel_id: Option<ShortChannelId>,
    source: Option<PublicKey>,
    destination: Option<PublicKey>,
) -> Result<ListchannelsResponse, Error> {
    let mut rpc = ClnRpc::new(&rpc_path).await?;
    let listchannels_request = rpc
        .call(Request::ListChannels(ListchannelsRequest {
            short_channel_id,
            source,
            destination,
        }))
        .await
        .map_err(|e| anyhow!("Error calling list_channels: {:?}", e))?;
    match listchannels_request {
        Response::ListChannels(info) => Ok(info),
        e => Err(anyhow!("Unexpected result in list_channels: {:?}", e)),
    }
}
