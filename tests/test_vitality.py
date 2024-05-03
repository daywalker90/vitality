#!/usr/bin/python
import os
from pathlib import Path

from pyln.testing.fixtures import *  # noqa: F403
from pyln.testing.utils import sync_blockheight, wait_for
from util import get_plugin  # noqa: F401


def test_basic(node_factory, bitcoind, get_plugin):  # noqa: F811
    os.environ["TEST_DEBUG"] = "true"
    l1, l2 = node_factory.get_nodes(2)
    l2.fundwallet(10_000_000)
    l2.rpc.fundchannel(
        l1.info["id"] + "@localhost:" + str(l1.port),
        1_000_000,
        mindepth=1,
        announce=True,
    )
    bitcoind.generate_block(6)
    sync_blockheight(bitcoind, [l1, l2])

    wait_for(
        lambda: len(l1.rpc.listpeerchannels(l2.info["id"])["channels"]) > 0
    )
    scid = l1.rpc.listpeerchannels(l2.info["id"])["channels"][0][
        "short_channel_id"
    ]
    wait_for(lambda: len(l1.rpc.listchannels(str(scid))["channels"]) == 2)
    wait_for(
        lambda: all(
            chan["public"]
            for chan in l1.rpc.listchannels(str(scid))["channels"]
        )
    )
    wait_for(
        lambda: all(
            chan["active"]
            for chan in l1.rpc.listchannels(str(scid))["channels"]
        )
    )

    lightning_dir = Path(l1.rpc.call("getinfo")["lightning-dir"])
    config_file = lightning_dir / "config"
    option_lines = [
        "vitality-amboss=true\n",
        "vitality-expiring-htlcs=50\n",
        "vitality-watch-channels=true\n",
        "vitality-watch-gossip=true\n",
        "vitality-telegram-token=4582169472:Og4grGKROE3OR-x-O3kfOsks\n",
        "vitality-telegram-usernames=936723718\n",
        "vitality-smtp-username=satoshi@gmx.de\n",
        "vitality-smtp-password=WEJF§IFJseo32\n",
        "vitality-smtp-server=mail.gmx.net\n",
        "vitality-smtp-port=587\n",
        "vitality-email-from=satoshi@gmx.de\n",
        "vitality-email-to=satoshi@gmx.de\n",
    ]

    with config_file.open(mode="a") as file:
        file.writelines(option_lines)

    l1.rpc.call("plugin", {"subcommand": "start", "plugin": str(get_plugin)})
    l1.daemon.wait_for_log(r"Error in amboss_ping")
    wait_for(lambda: l1.daemon.is_in_log(r"check_channel: All good."))
