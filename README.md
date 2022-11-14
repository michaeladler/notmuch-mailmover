[![Test](https://github.com/michaeladler/notmuch-mailmover/actions/workflows/test.yml/badge.svg)](https://github.com/michaeladler/notmuch-mailmover/actions/workflows/test.yml)
![Code Coverage](https://github.com/michaeladler/notmuch-mailmover/raw/ci/coverage/coverage.svg)

# notmuch-mailmover

notmuch-mailmover is a CLI application to assign notmuch *tagged* mails to IMAP *folders*.
For example, you can move all mails tagged as `trash` to the `Trash` folder.

It's written in Rust and thus, of course, *blazingly fast*.

## Use-Cases

* using IMAP folders you can sync your tags cross-device (less powerful than [muchsync](http://www.muchsync.org/) but
    easier to setup since you don't need an extra server)
* delete mail from the IMAP server (i.e., move trash mail to a non-synced folder and let offlineimap/isync do the rest)

## Installation

Only Linux is tested, but Windows and Mac *should* work as well.

If you use the [nix package manager](https://nixos.org/manual/nix/stable/), you can simply run

```bash
nix --extra-experimental-features 'nix-command flakes' \
    profile install 'github:michaeladler/notmuch-mailmover#default'
```

Otherwise, you have to build from source. You need the following build dependencies:

- Rust
- libnotmuch-dev

Then run

```bash
cargo install --git 'https://github.com/michaeladler/notmuch-mailmover/'
```

It's best to invoke `notmuch-mailmover` as a [notmuch pre-new hook](https://notmuch.readthedocs.io/en/latest/man5/notmuch-hooks.html).
You can also invoke `notmuch-mailmover` directly, but don't forget to run `notmuch new` afterward
(this isn't necessary if you add it as a pre-hook).

## Configuration

Running `notmuch-mailmover` for the first time will create `$XDG_CONFIG_HOME/notmuch-mailmover/config.yaml`.
Then adjust it to your needs, see below for an example.

## Example

Move all mails

* tagged as `trash` to folder `Trash`
* tagged as `sent` to folder `Sent`
* tagged as `archive` to folder `Archive`

```yml
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
(which I've been using for a long time) but arguably easier to configure because you don't have to configure the source
folders from which you want to move mails.
