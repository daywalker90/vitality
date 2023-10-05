# vitality
Core lightning (CLN) plugin to watch channel health, gossip health and ping amboss for online status

* [Installation](#installation)
* [Building](#building)
* [Usage](#usage)
* [Telegram](#telegram)
* [Options](#options)
* [Example](#example)

### Installation
For general plugin installation instructions see the plugins repo [README.md](https://github.com/lightningd/plugins/blob/master/README.md#Installation)

Release binaries for
* x86_64-linux
* armv7-linux (Raspberry Pi 32bit)
* aarch64-linux (Raspberry Pi 64bit)

can be found on the [release](https://github.com/daywalker90/vitality/releases) page. If you are unsure about your architecture you can run ``uname -m``.

They require ``glibc>=2.31``, which you can check with ``ldd --version``.

### Building
You can build the plugin yourself instead of using the release binaries.
First clone the repo:

``git clone https://github.com/daywalker90/vitality.git``

Install a recent rust version ([rustup](https://rustup.rs/) is recommended) and in the ``vitality`` folder run:

``cargo build --release``

After that the binary will be here: ``target/release/vitality``

### Usage
You can configure what the plugin checks for and optionally get notified with the options below.

These have to be in the ``config`` file in your ``lightning-dir`` (usually ``~/.lightning/config`` or ``~/.lightning/<network>/config``). The plugin is unable to read configs somewhere else, e.g. ``/etc/lightningd/config`` or from the cli.

The channel health checks happen a minute after start of the plugin and then every hour, so we don't disconnect from peers more than once an hour.

This is a dynamic plugin that can be started/stopped independently of CLN.

### Telegram
How to configure telegram notifications:
* Write to the @BotFather to create a bot and get the bot token
* Write to your bot
* visit ``https://api.telegram.org/bot<bottoken>/getUpdates`` and replace ``<bottoken>`` with your bot token
* get the chatid(s) that belong(s) to your username(s)/group(s) from the messages you see
* set the options for token and chatid(s) with the options below

### Options
* ``vitality-amboss`` ``default: false`` enable/disable pinging amboss for online status. Settings for online status visibility on your amboss page is here: [amboss](https://amboss.space/settings?page=monitoring)  Grace period needs to be 15min or higher, since we send every 5 minutes
* ``vitality-expiring-htlcs`` ``default: 0`` (off) check channels for expiring htlcs (with less than X blocks remaining) and does a reconnect in hope of fix
* ``vitality-watch-channels`` ``default: true`` check channels for errors in status, but not in closing state (sometimes needs manual force close), or disconnected peers that don't want to reconnect (e.g. can't agree on fees)
* ``vitality-watch-gossip`` ``default: false`` compare local channel info with local gossip info, checks for correct public and active values in gossip and missing gossip. Might get skipped if gossip content is low (e.g. lightningd deleted ``gossip.store`` or it got corrupted and is rebuilding). Does a reconnect in hope of fix
* ``vitality-telegram-token`` your telegram bot token
* ``vitality-telegram-usernames`` actually your chatid with the telegram bot, can be used multiple times
* ``vitality-smtp-username`` smtp username for email notifications
* ``vitality-smtp-password`` smtp password for email notifications
* ``vitality-smtp-server`` smtp server for email notifications
* ``vitality-smtp-port`` smtp server port for email notifications
* ``vitality-email-from`` email "from" field for email notifications
* ``vitality-email-to`` email to send to for email notifications

### Example
Example config with everything enabled, checking for htlcs that are closer than 50 blocks to expiry and notifications via telegram and email:
```
vitality-amboss=true
vitality-expiring-htlcs=50
vitality-watch-channels=true
vitality-watch-gossip=true
vitality-telegram-token=4582169472:Og4grGKROE3OR-x-O3kfOsks
vitality-telegram-usernames=936723718
vitality-smtp-username=satoshi@gmx.de
vitality-smtp-password=WEJF§IFJseo32
vitality-smtp-server=mail.gmx.net
vitality-smtp-port=587
vitality-email-from=satoshi@gmx.de
vitality-email-to=satoshi@gmx.de
```
