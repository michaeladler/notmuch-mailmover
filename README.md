[![Test](https://github.com/michaeladler/notmuch-mailmover/actions/workflows/test.yml/badge.svg)](https://github.com/michaeladler/notmuch-mailmover/actions/workflows/test.yml)
![Code Coverage](https://github.com/michaeladler/notmuch-mailmover/raw/ci/coverage/coverage.svg)

# notmuch-mailmover

notmuch-mailmover is a small tool to assign notmuch *tagged* mails to IMAP *folders*.
For example, you can move all mails tagged as `trash` to the `Trash` folder.

It's written in Rust and thus, of course, *blazingly fast*.

## Use-Cases

* using IMAP folders you can sync your tags cross-device (less powerful than [muchsync](http://www.muchsync.org/) but
    easier to setup since you don't need an extra server)
* delete mail from the IMAP server (i.e. move trash mail to a non-synced folder and let offlineimap/isync do the rest)

## Example

Move all mails with tag `trash` to folder `Trash` and mails with tag `archive` to folder `Archive`:

```yml
config_path: ~/.config/notmuch/default/config
database_path: ~/mail
# only rename if you use mbsync
rename: false
rules:
  - folder: Trash
    query: tag:trash
  - folder: Archive
    query: tag:archive and not tag:trash
```

**Note**: Rules **must not overlap** (hence the `and not tag:trash` clause in the second rule)!
This is checked by notmuch-mailmover *before* any files are moved.

## Similar Projects

This work is inspired by [afew's Mailmover plugin](https://github.com/afewmail/afew/blob/master/afew/MailMover.py)
(which I've been using for a long time) but (arguably) easier to configure.
