#!/usr/bin/python


from pyln.testing.fixtures import *  # noqa: F403
from util import get_plugin  # noqa: F401


def test_basic(node_factory, get_plugin):  # noqa: F811
    node = node_factory.get_node(
        options={"plugin": get_plugin, "vitality-fail": "lol"}
    )
