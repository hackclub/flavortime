# Flavortime
A Rust desktop app for Discord Rich Presence with Flavortown and Hack Club's referral program. Supports macOS (x86_64 and arm64), Windows (x86_64), and Linux (x64, relatively untested).

# What is Flavortown?
Check out https://flavortown.hackclub.com!

# But what exactly does this app do?
This app shows a rich presence for Discord that describes Flavortown and shows a button for other people to sign-up, which leads to a url with your referral code.
Example:

<img width="432" height="176" alt="image" src="https://github.com/user-attachments/assets/2b3db83d-7791-4706-8cc4-8ef4c1715f4d" />

> [!NOTE]
> The "Sign up" button only shows for other people, not for yourself.

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
