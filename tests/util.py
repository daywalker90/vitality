import os
from pathlib import Path

import pytest

RUST_PROFILE = os.environ.get("RUST_PROFILE", "debug")
plugin_dir = Path(__file__).parent.parent.resolve()
COMPILED_PATH = plugin_dir / "target" / RUST_PROFILE / "vitality"
DOWNLOAD_PATH = plugin_dir / "tests" / "vitality"


@pytest.fixture
def get_plugin(directory):
    if COMPILED_PATH.is_file():
        return COMPILED_PATH
    elif DOWNLOAD_PATH.is_file():
        return DOWNLOAD_PATH
    else:
        raise ValueError("No files were found.")
