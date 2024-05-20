#!/usr/bin/python
import os

import pytest
from pyln.client import RpcError
from pyln.testing.fixtures import *  # noqa: F403
from pyln.testing.utils import sync_blockheight, wait_for
from util import get_plugin  # noqa: F401


def test_basic(node_factory, bitcoind, get_plugin):  # noqa: F811
    os.environ["TEST_DEBUG"] = "true"
    l1_opts = {
        "plugin": get_plugin,
        "vitality-amboss": "true",
        "vitality-expiring-htlcs": "50",
        "vitality-watch-channels": "true",
        "vitality-watch-gossip": "true",
        "vitality-telegram-token": "4582169472:Og4grGKROE3OR-x-O3kfOsks",
        "vitality-telegram-usernames": "936723718",
        "vitality-smtp-username": "satoshi@gmx.de",
        "vitality-smtp-password": "WEJF§IFJseo32",
        "vitality-smtp-server": "mail.gmx.net",
        "vitality-smtp-port": "587",
        "vitality-email-from": "satoshi@gmx.de",
        "vitality-email-to": "satoshi@gmx.de",
    }
    l1, l2 = node_factory.get_nodes(2, opts=[l1_opts, {}])
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

    wait_for(lambda: l1.daemon.is_in_log(r"Error in amboss_ping"))
    wait_for(lambda: l1.daemon.is_in_log(r"check_channel: All good."))


def test_telegram_usernames(node_factory, get_plugin):  # noqa: F811
    os.environ["TEST_DEBUG"] = "true"

    l1 = node_factory.get_node(
        options={
            "plugin": get_plugin,
            "vitality-telegram-token": "4582169472:Og4grGKROE3OR-x-O3kfOsks",
            "vitality-telegram-usernames": "936723718,936723717",
        }
    )
    wait_for(
        lambda: l1.daemon.is_in_log(r"Will try to notify 936723718, 936723717")
    )


def test_options(node_factory, get_plugin):  # noqa: F811
    os.environ["TEST_DEBUG"] = "true"

    node = node_factory.get_node(
        options={
            "plugin": get_plugin,
            "vitality-smtp-port": 100000,
        }
    )
    assert node.daemon.is_in_log(
        r"out of range integral type conversion attempted"
    )

    node = node_factory.get_node(options={"plugin": get_plugin})

    node.rpc.setconfig("vitality-telegram-token", "test")
    assert (
        node.rpc.listconfigs("vitality-telegram-token")["configs"][
            "vitality-telegram-token"
        ]["value_str"]
        == "test"
    )

    node.rpc.setconfig("vitality-telegram-usernames", "userA, userB")
    assert (
        node.rpc.listconfigs("vitality-telegram-usernames")["configs"][
            "vitality-telegram-usernames"
        ]["value_str"]
        == "userA, userB"
    )

    with pytest.raises(RpcError, match="is not a valid integer"):
        node.rpc.setconfig("vitality-smtp-port", "test")
    with pytest.raises(
        RpcError, match="out of range integral type conversion attempted"
    ):
        node.rpc.setconfig("vitality-smtp-port", 99999)
    node.rpc.setconfig("vitality-smtp-port", 9999)

    node.rpc.setconfig("vitality-amboss", False)
    with pytest.raises(RpcError) as err:
        node.rpc.setconfig("vitality-amboss", "test")
    assert (
        err.value.error["message"] == "vitality-amboss is not a valid boolean!"
    )
    assert err.value.error["code"] == -32602
    assert (
        node.rpc.listconfigs("vitality-amboss")["configs"]["vitality-amboss"][
            "value_bool"
        ]
        != "test"
    )


def test_email_activation(node_factory, get_plugin):  # noqa: F811
    l1 = node_factory.get_node(
        options={
            "plugin": get_plugin,
            "vitality-smtp-username": "satoshi",
            "vitality-smtp-password": "password",
            "vitality-smtp-server": "mail.gmx.net",
            "vitality-smtp-port": 587,
            "vitality-email-from": "satoshi@gmx.net",
            "vitality-email-to": "hf@google.com",
        }
    )
    wait_for(
        lambda: l1.daemon.is_in_log(
            "plugin-vitality: Will try to send notifications via email"
        )
    )
