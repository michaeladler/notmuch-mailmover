#!/bin/sh
set -eux
cargo build --release
cp -a target/release/notmuch-mailmover dist/notmuch-mailmover_linux_amd64_v1/