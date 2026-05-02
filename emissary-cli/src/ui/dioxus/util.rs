// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use emissary_core::{
    crypto::{base32_encode, base64_decode},
    primitives::Destination,
};

use crate::{config::EmissaryConfig, ui::dioxus::LOG_TARGET};

use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
    sync::Arc,
};

/// Save router configuration to disk.
pub fn save_router_config(path: PathBuf, config: &EmissaryConfig) {
    if let Ok(serialized) = toml::to_string(config) {
        let _ = std::fs::write(path, serialized);
    }
}

/// Load base32 addresses from disk.
pub fn load_addresses(path: PathBuf) -> BTreeMap<Arc<str>, Arc<str>> {
    let Ok(file) = File::open(&path) else {
        tracing::warn!(
            target: LOG_TARGET,
            path = %path.display(),
            "failed to open address book file",
        );
        return BTreeMap::new();
    };
    let reader = BufReader::new(file);

    reader
        .lines()
        .filter_map(|line| {
            let line = line.ok()?;
            let (key, value) = line.split_once('=')?;
            Some((Arc::from(key), Arc::from(format!("http://{value}.b32.i2p"))))
        })
        .collect()
}

/// Trim hidden service display name.
///
/// Strips unnecessary prefixes and truncates the base32 address.
pub fn trim_display_name(s: &str) -> String {
    let s = s.strip_prefix("http://").unwrap_or(s);
    let s = s.strip_prefix("https://").unwrap_or(s);
    let s = s.strip_prefix("www").unwrap_or(s);
    let max_len = 50;
    if s.len() <= max_len {
        return s.to_string();
    }
    let keep = max_len - 3;
    let front = keep / 2;
    let back = keep - front;

    format!("{}{}{}", &s[..front], "...", &s[s.len() - back..])
}

/// Convert key file into a base32 address.
pub fn read_b32_address(path: &str) -> Option<String> {
    let data = std::fs::read_to_string(path).ok()?;
    let decoded = base64_decode(data.trim())?;
    let destination = Destination::parse(&decoded).ok()?;

    Some(base32_encode(destination.id().to_vec()))
}
