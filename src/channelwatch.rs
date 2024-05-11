use std::{collections::HashMap, env, time::Duration};

use anyhow::{anyhow, Error};
use cln_plugin::Plugin;

use cln_rpc::{
    model::{
        requests::{
            ConnectRequest, DisconnectRequest, GetinfoRequest, ListchannelsRequest,
            ListnodesRequest, ListpeerchannelsRequest,
        },
        responses::{
            ListchannelsChannels, ListpeerchannelsChannels, ListpeerchannelsChannelsState,
        },
    },
    primitives::{PublicKey, ShortChannelId},
    ClnRpc,
};
use log::{debug, info, warn};
use tokio::time::{self, Instant};

use crate::{
    structs::{Config, PluginState},
    util::{make_rpc_path, parse_boolean, send_mail, send_telegram},
};

async fn check_channel(plugin: Plugin<PluginState>) -> Result<(), Error> {
    let now = Instant::now();
    info!("check_channel: Starting");

    let rpc_path = make_rpc_path(&plugin);
    let mut rpc = ClnRpc::new(&rpc_path).await?;

    let channels = rpc
        .call_typed(&ListpeerchannelsRequest { id: None })
        .await?
        .channels
        .ok_or(anyhow!("No channels found"))?;
    info!("check_channel: Got state of all local channels");

    let config = plugin.state().config.lock().clone();

    let get_info = rpc.call_typed(&GetinfoRequest {}).await?;

    let current_blockheight = get_info.blockheight;

    let list_nodes = rpc.call_typed(&ListnodesRequest { id: None }).await?.nodes;
    let alias_map = list_nodes
        .into_iter()
        .filter_map(|a| a.alias.map(|alias| (a.nodeid, alias)))
        .collect::<HashMap<PublicKey, String>>();

    let gossip = if config.watch_gossip.value {
        Some(get_gossip_map(&mut rpc, get_info.id).await?)
    } else {
        None
    };
    let mut peer_slackers: HashMap<PublicKey, Vec<String>> = HashMap::new();

    check_slackers(
        &channels,
        &config,
        &mut peer_slackers,
        current_blockheight,
        &gossip,
    );

    let peer_map = channels
        .into_iter()
        .filter_map(|channel| channel.peer_id.map(|id| (id, channel)))
        .collect::<HashMap<PublicKey, ListpeerchannelsChannels>>();
    for (peer, status) in peer_slackers.iter_mut() {
        let connected = if let Some(p) = peer_map.get(peer) {
            p.peer_connected.unwrap()
        } else {
            continue;
        };
        if connected {
            info!("check_channel: disconnecting from: {}", peer);
            match rpc
                .call_typed(&DisconnectRequest {
                    id: *peer,
                    force: Some(true),
                })
                .await
            {
                Ok(_) => {
                    info!("check_channel: disconnect successful");
                }
                Err(de) => {
                    info!(
                        "check_channel: Could not disconnect from {}: {}",
                        peer, de.message
                    );
                    status.push(format!("Could not disconnect: {}", de.message));
                }
            };
        } else {
            info!("check_channel: already disconnected from: {}", peer);
        }
    }

    if !peer_slackers.is_empty() {
        info!("check_channel: Waiting 10s");
        time::sleep(Duration::from_secs(10)).await;
    }

    for (peer, status) in peer_slackers.iter_mut() {
        match rpc
            .call_typed(&ConnectRequest {
                id: peer.to_string(),
                host: None,
                port: None,
            })
            .await
        {
            Ok(_o) => {
                info!("check_channel: connect successful: {}", peer);
            }
            Err(ce) => {
                info!(
                    "check_channel: Could not connect to {}: {}",
                    peer, ce.message
                );
                status.push(format!("Could not connect: {}", ce.message));
            }
        }
    }

    if !peer_slackers.is_empty() {
        info!("check_channel: Waiting 30s");
        time::sleep(Duration::from_secs(30)).await;
    }

    let channels = rpc
        .call_typed(&ListpeerchannelsRequest { id: None })
        .await?
        .channels
        .ok_or(anyhow!("No channels found"))?;
    let gossip = if config.watch_gossip.value {
        Some(get_gossip_map(&mut rpc, get_info.id).await?)
    } else {
        None
    };
    let mut peer_slackers: HashMap<PublicKey, Vec<String>> = HashMap::new();

    check_slackers(
        &channels,
        &config,
        &mut peer_slackers,
        current_blockheight,
        &gossip,
    );

    if !peer_slackers.is_empty() {
        let final_peer_slackers: Vec<String> = peer_slackers
            .into_iter()
            .map(|(p, s)| {
                let concatenated_string = s.join("\n");
                if let Some(alias) = alias_map.get(&p) {
                    format!("{} ({}):\n{}\n", p, alias, concatenated_string)
                } else {
                    format!("{}:\n{}\n", p, concatenated_string)
                }
            })
            .collect();
        info!(
            "check_channel: Sending notifications. Duration: {}s",
            now.elapsed().as_secs()
        );
        let subject = "Channel check report\n".to_string();
        let body = final_peer_slackers.join("\n");
        if config.send_mail {
            send_mail(&config, &subject, &body, false).await?;
        }
        if config.send_telegram {
            send_telegram(&config, &subject, &body).await?;
        }
    } else {
        info!(
            "check_channel: All good. Duration: {}s",
            now.elapsed().as_secs()
        );
    }

    Ok(())
}

fn check_slackers(
    channels: &Vec<ListpeerchannelsChannels>,
    config: &Config,
    peer_slackers: &mut HashMap<PublicKey, Vec<String>>,
    current_blockheight: u32,
    gossip: &Option<HashMap<ShortChannelId, Vec<ListchannelsChannels>>>,
) {
    for chan in channels {
        let state = if let Some(st) = chan.state {
            st
        } else {
            continue;
        };

        match state {
            ListpeerchannelsChannelsState::CHANNELD_NORMAL
            | ListpeerchannelsChannelsState::CHANNELD_AWAITING_SPLICE => {
                let connected = chan.peer_connected.unwrap();
                let peer_id = chan.peer_id.unwrap();
                if config.watch_channels.value {
                    let statuses = chan.status.as_ref().unwrap();
                    let mut contained_reconnect = false;
                    let mut specific_error_found = false;
                    for status in statuses {
                        if status.to_lowercase().contains("error") {
                            warn!(
                                "check_channel: Found peer with error in status but not \
                                in closing state: {} status: {}",
                                peer_id, status
                            );
                            update_slackers(
                                peer_slackers,
                                peer_id,
                                format!(
                                    "Found peer with error in status but not \
                                in closing state. Status: {}",
                                    status
                                ),
                            );
                            specific_error_found = true;
                        }
                        if status.to_lowercase().contains("update_fee") {
                            warn!(
                                "check_channel: Can't agree on fee with: {} status: {}",
                                peer_id, status
                            );
                            update_slackers(
                                peer_slackers,
                                peer_id,
                                format!("Can't agree on fee. Status: {}", status),
                            );
                            specific_error_found = true;
                        }
                        if status.to_lowercase().contains("htlc") {
                            warn!("check_channel: {} status: {}", peer_id, status);
                            update_slackers(peer_slackers, peer_id, format!("Status: {}", status));
                            specific_error_found = true;
                        }
                        if status.to_lowercase().contains("will attempt reconnect") {
                            contained_reconnect = true;
                        }
                    }
                    if let Some(lost_state) = chan.lost_state {
                        if lost_state {
                            warn!(
                                "check_channel: Lost state with: {} status: \
                                we are fallen behind i.e. lost some channel state",
                                peer_id
                            );
                            update_slackers(
                                peer_slackers,
                                peer_id,
                                ("Lost state. Status: we are fallen behind \
                                i.e. lost some channel state")
                                    .to_string(),
                            );
                            specific_error_found = true;
                        }
                    }
                    if !connected && !contained_reconnect && !specific_error_found {
                        warn!(
                            "check_channel: Found disconnected peer that does not want to \
                            reconnect: {} status instead is: {}",
                            peer_id,
                            statuses.join("\n")
                        );
                        update_slackers(
                            peer_slackers,
                            peer_id,
                            format!(
                                "Found disconnected peer that does not want to \
                            reconnect. Status instead is: {}",
                                statuses.join("\n")
                            ),
                        );
                    }
                }
                if config.expiring_htlcs.value > 0 {
                    let htlcs = chan.htlcs.as_ref().unwrap();
                    for htlc in htlcs {
                        if let Some(expiry) = htlc.expiry {
                            if expiry - current_blockheight < config.expiring_htlcs.value {
                                warn!(
                                    "check_channel: Found peer {} with channel {} with close \
                                    to expiry htlc: {} blocks",
                                    peer_id,
                                    chan.short_channel_id.unwrap(),
                                    expiry - current_blockheight
                                );
                                update_slackers(
                                    peer_slackers,
                                    peer_id,
                                    format!(
                                        "Found channel {} with close to expiry htlc: {} blocks",
                                        chan.short_channel_id.unwrap(),
                                        expiry - current_blockheight
                                    ),
                                );
                            }
                        }
                    }
                }
                if let Some(goss) = &gossip {
                    let public = !chan.private.unwrap();
                    if !connected {
                        continue;
                    }
                    if goss.len()
                        < channels
                            .iter()
                            .filter(|s| s.private.is_some() && !s.private.unwrap())
                            .count()
                            / 2
                    {
                        warn!("check_channel: gossip_store still too empty...");
                        continue;
                    }
                    let chan_goss = goss.get(&chan.short_channel_id.unwrap());

                    if let Some(chan_gossip) = chan_goss {
                        if chan_gossip.len() == 1 {
                            warn!(
                                "check_channel: Found connected peer {} with channel {} \
                                    with one-sided gossip",
                                peer_id,
                                chan.short_channel_id.unwrap()
                            );
                            update_slackers(
                                peer_slackers,
                                peer_id,
                                format!(
                                    "Found connected channel {} with one-sided gossip",
                                    chan.short_channel_id.unwrap()
                                ),
                            );
                        } else {
                            for side in chan_gossip {
                                if !side.active {
                                    warn!(
                                        "check_channel: Found connected peer {} with channel {} \
                                        with inactive gossip",
                                        peer_id,
                                        chan.short_channel_id.unwrap()
                                    );
                                    update_slackers(
                                        peer_slackers,
                                        peer_id,
                                        format!(
                                            "Found connected channel {} with inactive gossip",
                                            chan.short_channel_id.unwrap()
                                        ),
                                    );
                                }
                                if public && !side.public {
                                    warn!(
                                        "check_channel: Found public peer {} with channel {} \
                                        with non-public gossip",
                                        peer_id,
                                        chan.short_channel_id.unwrap()
                                    );
                                    update_slackers(
                                        peer_slackers,
                                        peer_id,
                                        format!(
                                            "Found public channel {} with non-public gossip",
                                            chan.short_channel_id.unwrap()
                                        ),
                                    );
                                }
                            }
                        }
                    } else {
                        warn!(
                            "check_channel: Found peer {} with channel {} with no gossip",
                            peer_id,
                            chan.short_channel_id.unwrap()
                        );
                        update_slackers(
                            peer_slackers,
                            peer_id,
                            format!(
                                "Found channel {} with no gossip",
                                chan.short_channel_id.unwrap()
                            ),
                        );
                    }
                }
            }
            _ => continue,
        }
    }
}

async fn get_gossip_map(
    rpc: &mut ClnRpc,
    my_pubkey: PublicKey,
) -> Result<HashMap<ShortChannelId, Vec<ListchannelsChannels>>, Error> {
    let now = Instant::now();
    debug!("check_channel: getting our gossip...");
    let mut map: HashMap<ShortChannelId, Vec<ListchannelsChannels>> = HashMap::new();
    for list_channels in rpc
        .call_typed(&ListchannelsRequest {
            short_channel_id: None,
            source: Some(my_pubkey),
            destination: None,
        })
        .await?
        .channels
    {
        if let Some(existing_list) = map.get_mut(&list_channels.short_channel_id) {
            existing_list.push(list_channels);
        } else {
            map.insert(list_channels.short_channel_id, vec![list_channels]);
        }
    }
    for list_channels in rpc
        .call_typed(&ListchannelsRequest {
            short_channel_id: None,
            source: None,
            destination: Some(my_pubkey),
        })
        .await?
        .channels
    {
        if let Some(existing_list) = map.get_mut(&list_channels.short_channel_id) {
            existing_list.push(list_channels);
        } else {
            map.insert(list_channels.short_channel_id, vec![list_channels]);
        }
    }
    debug!(
        "check_channel: got our gossip in {}ms, gossip size: {}",
        now.elapsed().as_millis(),
        map.len()
    );
    Ok(map)
}

fn update_slackers(
    peer_slackers: &mut HashMap<PublicKey, Vec<String>>,
    peer_id: PublicKey,
    status: String,
) {
    if let Some(slack) = peer_slackers.get_mut(&peer_id) {
        slack.push(status)
    } else {
        peer_slackers.insert(peer_id, vec![status]);
    }
}

pub async fn check_channels_loop(plugin: Plugin<PluginState>) -> Result<(), Error> {
    let mut skip_sleep = false;
    if let Ok(dbg) = env::var("TEST_DEBUG") {
        if let Some(bl) = parse_boolean(&dbg) {
            if bl {
                skip_sleep = true
            }
        }
    }
    if !skip_sleep {
        time::sleep(Duration::from_secs(600)).await;
    }

    loop {
        {
            match check_channel(plugin.clone()).await {
                Ok(_succ) => (),
                Err(e) => {
                    warn!("Error in check_channel: {}", e.to_string());
                    let config = plugin.state().config.lock().clone();
                    let subject = "Channel check error".to_string();
                    let body = e.to_string();
                    if config.send_mail {
                        if let Err(e) = send_mail(&config, &subject, &body, false).await {
                            warn!("check_channels_loop: Error sending mail: {}", e);
                        };
                    }
                    if config.send_telegram {
                        if let Err(e) = send_telegram(&config, &subject, &body).await {
                            warn!("check_channels_loop: Error sending telegram: {}", e);
                        };
                    }
                }
            };
        }
        time::sleep(Duration::from_secs(3_600)).await;
    }
}
