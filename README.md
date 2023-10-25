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

Otherwise, you have to build from source. You need the following build dependencies:

- Rust
- libnotmuch-dev

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

## Example

Move all mails

* tagged as `trash` to folder `Trash`
* tagged as `sent` to folder `Sent`
* tagged as `archive` to folder `Archive`

`config.yaml`:

```yaml
maildir: ~/mail
# if omitted (or null), it will use the same as notmuch would, see notmuch-config(1)
notmuch_config: ~/.config/notmuch/notmuchrc
# only rename if you use mbsync
rename: true
rules:
  - folder: Trash
    query: tag:trash
  - folder: Sent
    query: tag:sent and not tag:trash
  - folder: Archive
    query: tag:archive and not tag:sent and not tag:trash
```

**Note**: Queries **must not overlap** (hence the `and not tag:trash` clause in the second query).
This is to avoid moving files more than once and checked by notmuch-mailmover *before* any files are moved.
So, don't worry about it, notmuch-mailmover will complain if your rules are ambiguous.

## Similar Projects

This work is inspired by [afew's Mailmover plugin](https://github.com/afewmail/afew/blob/master/afew/MailMover.py)
but doesn't require you to setup rules for each folder *individually*. Instead, notmuch-mailmover applies your rules
*once* to all folders (so it may be easier to configure if you have many folders).

