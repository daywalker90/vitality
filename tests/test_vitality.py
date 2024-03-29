#!/usr/bin/python
import time
from pathlib import Path

from pyln.testing.fixtures import *  # noqa: F403
from util import get_plugin  # noqa: F401


def test_basic(node_factory, get_plugin):  # noqa: F811
    node = node_factory.get_node()
    lightning_dir = Path(node.rpc.call("getinfo")["lightning-dir"])
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

    node.rpc.call("plugin", {"subcommand": "start", "plugin": str(get_plugin)})
    node.daemon.wait_for_log(r"Error in amboss_ping")
    time.sleep(5)
    assert node.daemon.is_in_log(r"check_channel: All good.")
