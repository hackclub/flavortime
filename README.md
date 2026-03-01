# Flavortime
A Rust desktop app for Discord Rich Presence with Flavortown and Hack Club's referral program. Supports macOS (x86_64 and arm64), Windows (x86_64), and Linux (x64, relatively untested).

# What is Flavortown?
Check out https://flavortown.hackclub.com!

# How do I build it?
```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh # install the relevant toolchains
cargo install tauri-cli
cargo tauri build # or cargo tauri dev
```

# A Linux build note:
```
NO_STRIP=true cargo tauri build --no-sign
```
- `NO_STRIP=true` skips the binary stripping step, which can avoid `failed to run linuxdeploy` AppImage bundling failures if you are facing them. Context: [tauri-apps/tauri issue #8929](https://github.com/tauri-apps/tauri/issues/8929).
- `--no-sign` fixes the error saying that `TAURI_SIGNING_PRIVATE_KEY` has not been set locally.

# Where can I install it?
You can download compiled executables on this [page](https://github.com/hackclub/flavortime/releases).

# Minimum Supported Rust Version
- MSRV: Rust 1.88.0 (measured with `cargo msrv find`)

# License
This project is dual-licensed under either of:

- MIT License ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.
