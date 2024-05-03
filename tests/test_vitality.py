#!/usr/bin/python
import os
import time
from pathlib import Path

from pyln.testing.fixtures import *  # noqa: F403
from util import get_plugin  # noqa: F401


def test_basic(node_factory, bitcoind, get_plugin):  # noqa: F811
    os.environ["TEST_DEBUG"] = "true"
    l1, l2 = node_factory.get_nodes(2)
    l1.rpc.connect(l2.info["id"], "localhost", l2.port)
    cl1, _ = l1.fundchannel(l2, 1_000_000)
    cl2, _ = l1.fundchannel(l2, 1_000_000)

    bitcoind.generate_block(6)

    l1.wait_channel_active(cl1)
    l1.wait_channel_active(cl2)

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
    time.sleep(5)
    assert l1.daemon.is_in_log(r"check_channel: All good.")
