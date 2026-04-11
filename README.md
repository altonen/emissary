## emissary

[![License](https://img.shields.io/badge/License-MIT-blue.svg)](https://github.com/eepnet/emissary/blob/master/LICENSE) [![Crates.io](https://img.shields.io/crates/v/emissary-core.svg)](https://crates.io/crates/emissary-core) [![docs.rs](https://img.shields.io/docsrs/emissary-core.svg)](https://docs.rs/emissary-core/latest/emissary_core/)

**`emissary` is a lightweight and embeddable [I2P](https://i2p.net/) router**

![UI](docs/router-ui.webp)

### Features

* Transports:
  * NTCP2
  * SSU2
* Client protocols:
  * I2CP
  * SAMv3
* Proxies:
  * HTTP
  * SOCKSv5

### Directory layout

* `emissary-core/` - I2P protocol implementation as an asynchronous library
* `emissary-util/` - `emissary-core`-related utilities, such as runtime implementations and reseeder
* `emissary-cli/` - `tokio`-based I2P router implementation

### Usage

1) Install from [crates.io](https://crates.io/crates/emissary-cli): `cargo install emissary-cli`
2) Build from sources: `cargo build --release`

Router installs its files under `$HOME/.emissary`, automatically reseeds over HTTPS on first boot and creates a default configuration. For instructions on how to browse and host eepsites, use torrents, or chat on Irc2P, visit [documentation](https://eepnet.github.io/emissary/).

### Third-Party Licenses

Google Material Icons (optional), Apache 2.0
