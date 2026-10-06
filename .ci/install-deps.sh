#!/bin/sh
set -eux
apt-get update -q
apt-get install -y libnotmuch-dev liblua5.4-dev
