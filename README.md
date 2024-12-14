[![CI](https://github.com/michaeladler/notmuch-mailmover/actions/workflows/ci.yml/badge.svg)](https://github.com/michaeladler/notmuch-mailmover/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/michaeladler/notmuch-mailmover/graph/badge.svg?token=6U7O3F51P7)](https://codecov.io/gh/michaeladler/notmuch-mailmover)

# notmuch-mailmover

notmuch-mailmover is a tool to move [notmuch](https://notmuchmail.org/) tagged mails into [Maildir](https://en.wikipedia.org/wiki/Maildir) folders (e.g., created by [offlineimap](https://github.com/OfflineIMAP/offlineimap3)/[mbsync](https://isync.sourceforge.io/)).

For example, you can move all mails tagged as `trash` to the `Trash` folder.

## Use-Cases

Some use-cases are:

* delete mail from IMAP server (e.g. move trash mail to a non-synced folder and let offlineimap/mbsync do the rest)
* sync your notmuch tags across devices by using notmuch-mailmover in combination with offlineimap/mbsync
  (this is similarly to [muchsync](http://www.muchsync.org/) but easier to setup since you don't need a muchsync server)
* purge old mails from the IMAP server (by moving them out of synced folders)

## Installation

Only Linux is tested, but Windows and Mac *should* work as well.

### Arch Linux

[notmuch-mailmover-git](https://aur.archlinux.org/packages/notmuch-mailmover-git) is available on the AUR.
Use your favorite AUR helper to install it, e.g.

```bash
$ yay -S notmuch-mailmover-git
```

### NixOS

```bash
$ nix-env -iA notmuch-mailmover
```

### Building from Source

Otherwise, you have to build from source. You need the following build dependencies (see [install-deps.sh](.ci/install-deps.sh)):

- Rust
- libnotmuch-dev
- liblua5.4-dev

Then run

```bash
cargo install --git 'https://github.com/michaeladler/notmuch-mailmover/'
```

## Setup

It's recommended to run `notmuch-mailmover` as part of your [notmuch pre-new hook](https://notmuch.readthedocs.io/en/latest/man5/notmuch-hooks.html).
You can also invoke `notmuch-mailmover` directly, but don't forget to run `notmuch new` afterward (this isn't necessary if you add it as a pre-hook).

## Configuration

Running `notmuch-mailmover` for the first time will create `$XDG_CONFIG_HOME/notmuch-mailmover/config.yaml`.
Then edit the file as you like, see below for an example.
Alternatively, it's possible to write the configuration in Lua 5.4 by creating `config.lua` instead.
See the provided [config.lua](example/config.lua) for an example.

The configuration is largely self-explanatory, except perhaps for the choice of the `rule_match_mode`.
You need to decide whether you want your rules to be pairwise distinct (meaning the queries must not overlap) or ambiguous (where the first or last matching rule wins).
The `unique` approach is more explicit but also more verbose, while the `first` or `all` approach is more concise but may lead to unexpected behavior if you have overlapping rules, as the order of the rules matters.

The `unique` match-mode also allows to disambiguate message files based on a
prefix of their path, bypassing a limitation of `folder:` and `path:` `notmuch`
search terms.

## Example

The provided [config.yaml](./example/config.yaml) does the following:

* move mails tagged as `trash` to folder `Trash`
* move mails tagged as `sent` to folder `Sent`
* move mails tagged as `archive` to folder `Archive`

See [config_first.yaml](./example/config_first.yaml) for a different approach (using the `first` strategy for `rule_match_mode`).

## Similar Projects

This work is inspired by [afew's Mailmover plugin](https://github.com/afewmail/afew/blob/master/afew/MailMover.py)
but doesn't require you to setup rules for each folder *individually*. Instead, notmuch-mailmover applies your rules
*once* to all folders (so it may be easier to configure if you have many folders).
