use std::{collections::HashMap, env, time::Duration};

use anyhow::{Error, anyhow};
use cln_plugin::Plugin;
use cln_rpc::{
    ClnRpc,
    model::{
        requests::{
            ConnectRequest,
            DisconnectRequest,
            GetinfoRequest,
            ListchannelsRequest,
            ListnodesRequest,
            ListpeerchannelsRequest,
        },
        responses::{ListchannelsChannels, ListpeerchannelsChannels},
    },
    primitives::{ChannelState, PublicKey, ShortChannelId},
};
use log::{debug, info, warn};
use tokio::time::{self, Instant};

use crate::{
    structs::{Config, PluginState},
    util::{make_rpc_path, parse_boolean, send_mail},
};

#[allow(clippy::too_many_lines)]
async fn check_channel(plugin: Plugin<PluginState>) -> Result<(), Error> {
    let now = Instant::now();
    info!("check_channel: Starting");

    let rpc_path = make_rpc_path(&plugin);
    let mut rpc = ClnRpc::new(&rpc_path).await?;

    let channels = rpc
        .call_typed(&ListpeerchannelsRequest {
            id: None,
            short_channel_id: None,
            channel_id: None,
        })
        .await?
        .channels;
    info!("check_channel: Got state of all local channels");

    let config = plugin.state().config.lock().clone();

    let get_info = rpc.call_typed(&GetinfoRequest {}).await?;

    let current_blockheight = get_info.blockheight;

    let list_nodes = rpc.call_typed(&ListnodesRequest { id: None }).await?.nodes;
    let alias_map = list_nodes
        .into_iter()
        .filter_map(|a| a.alias.map(|alias| (a.nodeid, alias)))
        .collect::<HashMap<PublicKey, String>>();

    let gossip = if config.watch_gossip {
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
        gossip.as_ref(),
    )?;

    let peer_map = channels
        .into_iter()
        .map(|channel| (channel.peer_id, channel))
        .collect::<HashMap<PublicKey, ListpeerchannelsChannels>>();
    for (peer, status) in &mut peer_slackers {
        let connected = if let Some(p) = peer_map.get(peer) {
            p.peer_connected
        } else {
            continue;
        };
        if connected {
            info!("check_channel: disconnecting from: {peer}");
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
            }
        } else {
            info!("check_channel: already disconnected from: {peer}");
        }
    }

    if !peer_slackers.is_empty() {
        info!("check_channel: Waiting 10s");
        time::sleep(Duration::from_secs(10)).await;
    }

    for (peer, status) in &mut peer_slackers {
        match rpc
            .call_typed(&ConnectRequest {
                id: peer.to_string(),
                host: None,
                port: None,
            })
            .await
        {
            Ok(_o) => {
                info!("check_channel: connect successful: {peer}");
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
        .call_typed(&ListpeerchannelsRequest {
            id: None,
            short_channel_id: None,
            channel_id: None,
        })
        .await?
        .channels;
    let gossip = if config.watch_gossip {
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
        gossip.as_ref(),
    )?;

    if peer_slackers.is_empty() {
        info!(
            "check_channel: All good. Duration: {}s",
            now.elapsed().as_secs()
        );
    } else {
        let final_peer_slackers: Vec<String> = peer_slackers
            .into_iter()
            .map(|(p, s)| {
                let concatenated_string = s.join("\n");
                if let Some(alias) = alias_map.get(&p) {
                    format!("{p} ({alias}):\n{concatenated_string}\n")
                } else {
                    format!("{p}:\n{concatenated_string}\n")
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
            send_mail(&config, subject, body, false).await?;
        }
    }

    Ok(())
}

#[allow(clippy::too_many_lines)]
fn check_slackers(
    channels: &Vec<ListpeerchannelsChannels>,
    config: &Config,
    peer_slackers: &mut HashMap<PublicKey, Vec<String>>,
    current_blockheight: u32,
    gossip: Option<&HashMap<ShortChannelId, Vec<ListchannelsChannels>>>,
) -> Result<(), anyhow::Error> {
    for chan in channels {
        match chan.state {
            ChannelState::CHANNELD_AWAITING_LOCKIN => {
                if config.watch_channels {
                    let statuses = chan
                        .status
                        .as_ref()
                        .ok_or_else(|| anyhow!("No status found"))?;
                    for status in statuses {
                        if status.contains("We've confirmed channel ready, they haven't yet.") {
                            warn!(
                                "check_channel: Peer won't lockin our channel: {} status: {}",
                                chan.peer_id, status
                            );
                            update_slackers(
                                peer_slackers,
                                chan.peer_id,
                                format!("Peer won't lockin our channel. Status: {status}"),
                            );
                        }
                        if status.contains("Sent reestablish, waiting for theirs") {
                            warn!(
                                "check_channel: Peer won't reestablish our channel: {} status: {}",
                                chan.peer_id, status
                            );
                            update_slackers(
                                peer_slackers,
                                chan.peer_id,
                                format!("Peer won't reestablish our channel. Status: {status}"),
                            );
                        }
                    }
                }
            }
            ChannelState::CHANNELD_NORMAL | ChannelState::CHANNELD_AWAITING_SPLICE => {
                if config.watch_channels {
                    let statuses = chan.status.as_ref().unwrap();
                    for status in statuses {
                        if status.to_lowercase().contains("error") {
                            warn!(
                                "check_channel: Found peer with error in status but not \
                                in closing state: {} status: {}",
                                chan.peer_id, status
                            );
                            update_slackers(
                                peer_slackers,
                                chan.peer_id,
                                format!(
                                    "Found peer with error in status but not \
                                in closing state. Status: {status}"
                                ),
                            );
                        }
                        if status.to_lowercase().contains("update_fee") {
                            warn!(
                                "check_channel: Can't agree on fee with: {} status: {}",
                                chan.peer_id, status
                            );
                            update_slackers(
                                peer_slackers,
                                chan.peer_id,
                                format!("Can't agree on fee. Status: {status}"),
                            );
                        }
                        if status.to_lowercase().contains("htlc") {
                            warn!("check_channel: {} status: {}", chan.peer_id, status);
                            update_slackers(
                                peer_slackers,
                                chan.peer_id,
                                format!("Status: {status}"),
                            );
                        }
                    }
                    if let Some(lost_state) = chan.lost_state {
                        if lost_state {
                            warn!(
                                "check_channel: Lost state with: {} status: \
                                we are fallen behind i.e. lost some channel state",
                                chan.peer_id
                            );
                            update_slackers(
                                peer_slackers,
                                chan.peer_id,
                                ("Lost state. Status: we are fallen behind \
                                i.e. lost some channel state")
                                    .to_string(),
                            );
                        }
                    }
                }
                if config.expiring_htlcs > 0 {
                    let htlcs = chan.htlcs.as_ref().unwrap();
                    for htlc in htlcs {
                        if htlc.expiry - current_blockheight < config.expiring_htlcs {
                            warn!(
                                "check_channel: Found peer {} with channel {} with close \
                                    to expiry htlc: {} blocks",
                                chan.peer_id,
                                chan.short_channel_id.unwrap(),
                                htlc.expiry - current_blockheight
                            );
                            update_slackers(
                                peer_slackers,
                                chan.peer_id,
                                format!(
                                    "Found channel {} with close to expiry htlc: {} blocks",
                                    chan.short_channel_id.unwrap(),
                                    htlc.expiry - current_blockheight
                                ),
                            );
                        }
                    }
                }
                if let Some(goss) = &gossip {
                    let public = !chan.private.unwrap();
                    if !chan.peer_connected {
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
                                chan.peer_id,
                                chan.short_channel_id.unwrap()
                            );
                            update_slackers(
                                peer_slackers,
                                chan.peer_id,
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
                                        chan.peer_id,
                                        chan.short_channel_id.unwrap()
                                    );
                                    update_slackers(
                                        peer_slackers,
                                        chan.peer_id,
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
                                        chan.peer_id,
                                        chan.short_channel_id.unwrap()
                                    );
                                    update_slackers(
                                        peer_slackers,
                                        chan.peer_id,
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
                            chan.peer_id,
                            chan.short_channel_id.unwrap()
                        );
                        update_slackers(
                            peer_slackers,
                            chan.peer_id,
                            format!(
                                "Found channel {} with no gossip",
                                chan.short_channel_id.unwrap()
                            ),
                        );
                    }
                }
            }
            _ => (),
        }
    }
    Ok(())
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
        slack.push(status);
    } else {
        peer_slackers.insert(peer_id, vec![status]);
    }
}

pub async fn check_channels_loop(plugin: Plugin<PluginState>) -> Result<(), Error> {
    let mut skip_sleep = false;
    if let Ok(dbg) = env::var("TEST_DEBUG") {
        if let Some(bl) = parse_boolean(&dbg) {
            if bl {
                skip_sleep = true;
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
                    warn!("Error in check_channel: {e}");
                    let config = plugin.state().config.lock().clone();
                    let subject = "Channel check error".to_string();
                    let body = e.to_string();
                    if config.send_mail {
                        if let Err(e) = send_mail(&config, subject, body, false).await {
                            warn!("check_channels_loop: Error sending mail: {e}");
                        }
                    }
                }
            }
        }
        time::sleep(Duration::from_secs(3_600)).await;
    }
}
