use std::{collections::HashMap, time::Duration};

use anyhow::{anyhow, Error};
use cln_plugin::Plugin;

use cln_rpc::{
    model::responses::{
        ListchannelsChannels, ListpeerchannelsChannels, ListpeerchannelsChannelsState,
    },
    primitives::{PublicKey, ShortChannelId},
    RpcError,
};
use log::{debug, info, warn};
use tokio::time::{self, Instant};

use crate::{
    rpc::{connect, disconnect, get_info, list_channels, list_peer_channels},
    structs::PluginState,
    util::{make_rpc_path, send_mail, send_telegram},
};

async fn check_channel(plugin: Plugin<PluginState>) -> Result<(), Error> {
    let now = Instant::now();
    info!("check_channel: Starting");

    let rpc_path = make_rpc_path(&plugin);

    let channels = list_peer_channels(&rpc_path, None)
        .await?
        .channels
        .ok_or(anyhow!("No channels found"))?;
    info!("check_channel: Got state of all local channels");

    let config = plugin.state().config.lock().clone();

    let get_info = get_info(&rpc_path).await?;

    let current_blockheight = get_info.blockheight;
    let network = get_info.network;

    debug!("check_channel: {}ms", now.elapsed().as_millis());

    let gossip = if config.watch_gossip.1 {
        debug!("check_channel: getting gossip...");
        let mut map: HashMap<ShortChannelId, Vec<ListchannelsChannels>> = HashMap::new();
        for list_channels in list_channels(&rpc_path, None, None, None).await?.channels {
            if let Some(existing_list) = map.get_mut(&list_channels.short_channel_id) {
                existing_list.push(list_channels);
            } else {
                map.insert(list_channels.short_channel_id, vec![list_channels]);
            }
        }
        debug!("check_channel: gossip size: {}", map.len());
        Some(map)
    } else {
        None
    };
    debug!("check_channel: {}ms", now.elapsed().as_millis());
    let mut peer_slackers: HashMap<PublicKey, Vec<String>> = HashMap::new();
    for chan in &channels {
        let state = if let Some(st) = chan.state {
            st
        } else {
            continue;
        };
        match state {
            ListpeerchannelsChannelsState::CHANNELD_NORMAL
            | ListpeerchannelsChannelsState::CHANNELD_AWAITING_SPLICE => {
                let connected = chan.peer_connected.unwrap();
                if config.watch_channels.1 {
                    let statuses = chan.status.as_ref().unwrap();
                    let mut contained_reconnect = false;
                    for status in statuses {
                        if status.to_lowercase().contains("error") {
                            warn!(
                                "check_channel: Found peer with error in status but not \
                                in closing state: {} status: {}",
                                chan.peer_id.unwrap().to_string(),
                                status
                            );
                            update_slackers(
                                &mut peer_slackers,
                                chan.peer_id.unwrap(),
                                format!(
                                    "Found peer with error in status but not \
                                in closing state. Status: {}",
                                    status
                                ),
                            );
                        }
                        if status.to_lowercase().contains("will attempt reconnect") {
                            contained_reconnect = true;
                        }
                    }
                    if !connected && !contained_reconnect {
                        warn!(
                            "check_channel: Found disconnected peer that does not want to \
                            reconnect: {} status instead is: {}",
                            chan.peer_id.unwrap().to_string(),
                            statuses.join("\n")
                        );
                        update_slackers(
                            &mut peer_slackers,
                            chan.peer_id.unwrap(),
                            format!(
                                "Found disconnected peer that does not want to \
                            reconnect. Status instead is: {}",
                                statuses.join("\n")
                            ),
                        );
                    }
                }
                if config.expiring_htlcs.1 > 0 {
                    let htlcs = chan.htlcs.as_ref().unwrap();
                    for htlc in htlcs {
                        if let Some(expiry) = htlc.expiry {
                            if expiry - current_blockheight < config.expiring_htlcs.1 {
                                warn!(
                                "check_channel: Found peer {} with close to expiry htlc: {} blocks",
                                chan.peer_id.unwrap().to_string(),
                                expiry - current_blockheight
                            );
                                update_slackers(
                                    &mut peer_slackers,
                                    chan.peer_id.unwrap(),
                                    format!(
                                        "Found peer with close to expiry htlc: {}",
                                        expiry - current_blockheight
                                    ),
                                );
                            }
                        }
                    }
                }
                if let Some(goss) = &gossip {
                    let public = !chan.private.unwrap();
                    if connected
                        && (goss.len() > 40_000 && network == "bitcoin"
                            || goss.len() > 2_000 && network == "testnet"
                            || network == "regtest")
                    {
                        let chan_goss = goss.get(&chan.short_channel_id.unwrap());

                        if chan_goss.is_none() {
                            warn!(
                                "check_channel: Found peer with no gossip: {}",
                                chan.peer_id.unwrap().to_string()
                            );
                            update_slackers(
                                &mut peer_slackers,
                                chan.peer_id.unwrap(),
                                "Found peer with no gossip".to_string(),
                            );
                        } else if chan_goss.unwrap().len() == 1 {
                            warn!(
                                "check_channel: Found connected peer with one-sided gossip: {}",
                                chan.peer_id.unwrap().to_string()
                            );
                            update_slackers(
                                &mut peer_slackers,
                                chan.peer_id.unwrap(),
                                "Found connected peer with one-sided gossip".to_string(),
                            );
                        } else {
                            for side in chan_goss.unwrap() {
                                if !side.active {
                                    warn!(
                                        "check_channel: Found connected peer with inactive \
                                            gossip: {}",
                                        chan.peer_id.unwrap().to_string()
                                    );
                                    update_slackers(
                                        &mut peer_slackers,
                                        chan.peer_id.unwrap(),
                                        "Found connected peer with inactive gossip".to_string(),
                                    );
                                }
                                if public && !side.public {
                                    warn!(
                                        "check_channel: Found public peer with non-public \
                                            gossip: {}",
                                        chan.peer_id.unwrap().to_string()
                                    );
                                    update_slackers(
                                        &mut peer_slackers,
                                        chan.peer_id.unwrap(),
                                        "Found public peer with non-public gossip".to_string(),
                                    );
                                }
                            }
                        }
                    }
                }
            }
            _ => continue,
        }
    }
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
            info!("check_channel: disconnecting from: {}", peer.to_string());
            match disconnect(&rpc_path, *peer).await {
                Ok(_) => {
                    info!("check_channel: disconnect successful, connecting");
                }
                Err(de) => match de.downcast() {
                    Ok(RpcError { code: _, message }) => {
                        info!(
                            "check_channel: Could not disconnect from {}: {}",
                            peer, message
                        );
                        status.push(format!("Could not disconnect: {}", message));
                    }
                    Err(e) => {
                        return Err(anyhow!(e));
                    }
                },
            };
        } else {
            info!("check_channel: already disconnected from: {}", peer);
        }
    }

    if !peer_slackers.is_empty() {
        time::sleep(Duration::from_secs(10)).await;
    }

    for (peer, status) in peer_slackers.iter_mut() {
        match connect(&rpc_path, *peer, None, None).await {
            Ok(_o) => {
                info!("check_channel: connect successful: {}", peer);
            }
            Err(ce) => match ce.downcast() {
                Ok(RpcError { code: _, message }) => {
                    info!("check_channel: Could not connect to {}: {}", peer, message);
                    status.push(format!("Could not connect: {}", message));
                }
                Err(e) => {
                    return Err(anyhow!(e));
                }
            },
        }
    }

    if !peer_slackers.is_empty() {
        time::sleep(Duration::from_secs(30)).await;
    }

    let peer_channels = list_peer_channels(&rpc_path, None).await?.channels.unwrap();
    let mut peer_map: HashMap<PublicKey, Vec<ListpeerchannelsChannels>> = HashMap::new();
    for chan in peer_channels {
        if let Some(public_key) = chan.peer_id {
            peer_map
                .entry(public_key)
                .and_modify(|vec| vec.push(chan.clone()))
                .or_insert(vec![chan]);
        }
    }
    for (peer, status) in peer_slackers.iter_mut() {
        let chans = if let Some(c) = peer_map.get(peer) {
            c
        } else {
            continue;
        };
        for chan in chans {
            let state = if let Some(st) = chan.state {
                st
            } else {
                continue;
            };
            match state {
                ListpeerchannelsChannelsState::CHANNELD_NORMAL
                | ListpeerchannelsChannelsState::CHANNELD_AWAITING_SPLICE => {
                    let htlcs = if let Some(h) = &chan.htlcs {
                        h
                    } else {
                        continue;
                    };
                    for htlc in htlcs {
                        let expiry = if let Some(exp) = htlc.expiry {
                            exp
                        } else {
                            continue;
                        };
                        if expiry - current_blockheight < config.expiring_htlcs.1 {
                            warn!(
                                "check_channel: Could not fix stuck htlc: {} \
                                            Blocks until fc in {}",
                                expiry - current_blockheight,
                                chan.short_channel_id.unwrap().to_string()
                            );
                            status.push(format!(
                                "Could not fix stuck htlc: {} Blocks until fc in {}",
                                expiry - current_blockheight,
                                chan.short_channel_id.unwrap().to_string()
                            ));
                        }
                    }
                }
                _ => continue,
            }
        }
    }

    if !peer_slackers.is_empty() {
        let final_peer_slackers: Vec<String> = peer_slackers
            .into_iter()
            .map(|(p, s)| {
                let concatenated_string = s.join("\n");
                p.to_string() + ":\n" + &concatenated_string + "\n"
            })
            .collect();
        info!(
            "check_channel: Sending mailreport. Duration: {}s",
            now.elapsed().as_secs()
        );
        let subject = "Channel check report".to_string();
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
    time::sleep(Duration::from_secs(60)).await;
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
                        send_mail(&config, &subject, &body, false).await?;
                    }
                    if config.send_telegram {
                        send_telegram(&config, &subject, &body).await?;
                    }
                }
            };
        }
        time::sleep(Duration::from_secs(3_600)).await;
    }
}
