#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:$PATH"
sudo apt-get update
sudo apt-get install -y build-essential
tools/package_native_release.sh --force
