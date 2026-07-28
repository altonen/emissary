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

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Exact Proposal 170 administrative address book type strings.
///
/// Each variant maps to exactly one external wire spelling. These represent
/// independent administrative stores, not the runtime resolver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AdministrativeAddressBookType {
    #[serde(rename = "private")]
    Private,
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "router")]
    Router,
    #[serde(rename = "published")]
    Published,
}

/// All valid administrative address book types in canonical order.
pub const ALL_ADDRESS_BOOK_TYPES: &[AdministrativeAddressBookType] = &[
    AdministrativeAddressBookType::Private,
    AdministrativeAddressBookType::Local,
    AdministrativeAddressBookType::Router,
    AdministrativeAddressBookType::Published,
];

impl AdministrativeAddressBookType {
    /// Return the exact external wire string for this book type.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::Local => "local",
            Self::Router => "router",
            Self::Published => "published",
        }
    }

    /// Parse from an exact external wire string.
    pub fn from_str_exact(s: &str) -> Option<Self> {
        match s {
            "private" => Some(Self::Private),
            "local" => Some(Self::Local),
            "router" => Some(Self::Router),
            "published" => Some(Self::Published),
            _ => None,
        }
    }
}

impl fmt::Display for AdministrativeAddressBookType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for AdministrativeAddressBookType {
    type Err = AddressBookTypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_str_exact(s).ok_or_else(|| AddressBookTypeError(s.to_string()))
    }
}

/// Error returned when a string is not a valid address book type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressBookTypeError(pub String);

impl fmt::Display for AddressBookTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid address book type {:?}; expected one of: {}",
            self.0,
            ALL_ADDRESS_BOOK_TYPES.iter().map(|t| t.as_str()).collect::<Vec<_>>().join(", ")
        )
    }
}

impl std::error::Error for AddressBookTypeError {}

/// An address book entry: a hostname mapped to a validated destination.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AddressBookEntry {
    /// The hostname (exact external spelling preserved).
    pub hostname: String,

    /// The validated destination (base64 or base32).
    pub destination: String,
}

impl AddressBookEntry {
    pub fn new(hostname: impl Into<String>, destination: impl Into<String>) -> Self {
        Self {
            hostname: hostname.into(),
            destination: destination.into(),
        }
    }
}

/// Ordered persistent subscription strings.
///
/// Subscriptions are stored as an ordered list of URL strings. Deterministic
/// ordering is maintained by storage order (insertion order preserved).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SubscriptionSet {
    subscriptions: Vec<String>,
}

impl SubscriptionSet {
    /// Create a new empty subscription set.
    pub fn new() -> Self {
        Self {
            subscriptions: Vec::new(),
        }
    }

    /// Create from an ordered list of subscriptions.
    pub fn from_vec(subscriptions: Vec<String>) -> Self {
        Self { subscriptions }
    }

    /// Return the subscriptions as a slice.
    pub fn as_slice(&self) -> &[String] {
        &self.subscriptions
    }

    /// Return true if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.subscriptions.is_empty()
    }

    /// Return the number of subscriptions.
    pub fn len(&self) -> usize {
        self.subscriptions.len()
    }

    /// Add a subscription to the set.
    pub fn push(&mut self, subscription: String) {
        if !self.subscriptions.contains(&subscription) {
            self.subscriptions.push(subscription);
        }
    }

    /// Remove a subscription from the set.
    pub fn remove(&mut self, subscription: &str) -> bool {
        let len_before = self.subscriptions.len();
        self.subscriptions.retain(|s| s != subscription);
        self.subscriptions.len() < len_before
    }

    /// Return true if the set contains the given subscription.
    pub fn contains(&self, subscription: &str) -> bool {
        self.subscriptions.contains(&subscription.to_string())
    }
}

impl Default for SubscriptionSet {
    fn default() -> Self {
        Self::new()
    }
}

/// Address book configuration as a string-keyed map with deterministic ordering.
///
/// The map is stored as a `BTreeMap` to guarantee deterministic serialization
/// regardless of insertion order.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AddressBookConfiguration {
    entries: BTreeMap<String, String>,
}

impl AddressBookConfiguration {
    /// Create a new empty configuration.
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Create from an existing map.
    pub fn from_map(entries: BTreeMap<String, String>) -> Self {
        Self { entries }
    }

    /// Return the configuration map as a reference.
    pub fn as_map(&self) -> &BTreeMap<String, String> {
        &self.entries
    }

    /// Return true if the configuration is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get a configuration value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(|s| s.as_str())
    }

    /// Set a configuration value.
    pub fn insert(&mut self, key: String, value: String) {
        self.entries.insert(key, value);
    }

    /// Remove a configuration value.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.entries.remove(key)
    }
}

impl Default for AddressBookConfiguration {
    fn default() -> Self {
        Self::new()
    }
}

/// Request mode for AddressBook operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum AddressBookRequest {
    #[serde(rename = "List")]
    List,
    #[serde(rename = "Lookup")]
    Lookup,
    #[serde(rename = "Add")]
    Add,
    #[serde(rename = "Update")]
    Update,
    #[serde(rename = "Delete")]
    Delete,
}

impl AddressBookRequest {
    /// Return the exact external wire string for this request.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::List => "List",
            Self::Lookup => "Lookup",
            Self::Add => "Add",
            Self::Update => "Update",
            Self::Delete => "Delete",
        }
    }

    /// Parse from an exact external wire string.
    pub fn from_str_exact(s: &str) -> Option<Self> {
        match s {
            "List" => Some(Self::List),
            "Lookup" => Some(Self::Lookup),
            "Add" => Some(Self::Add),
            "Update" => Some(Self::Update),
            "Delete" => Some(Self::Delete),
            _ => None,
        }
    }
}

impl fmt::Display for AddressBookRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_book_type_parse_all_variants() {
        for abt in ALL_ADDRESS_BOOK_TYPES {
            let s = abt.as_str();
            let parsed = AdministrativeAddressBookType::from_str_exact(s).expect(s);
            assert_eq!(&parsed, abt);
            assert_eq!(parsed.as_str(), s);
        }
    }

    #[test]
    fn address_book_type_reject_unknown() {
        assert!(AdministrativeAddressBookType::from_str_exact("unknown").is_none());
        assert!(AdministrativeAddressBookType::from_str_exact("Private").is_none());
        assert!(AdministrativeAddressBookType::from_str_exact("PRIVATE").is_none());
        assert!(AdministrativeAddressBookType::from_str_exact("").is_none());
    }

    #[test]
    fn address_book_type_serialization_exact() {
        let json = serde_json::to_string(&AdministrativeAddressBookType::Private).unwrap();
        assert_eq!(json, "\"private\"");
    }

    #[test]
    fn address_book_type_count() {
        assert_eq!(ALL_ADDRESS_BOOK_TYPES.len(), 4);
    }

    #[test]
    fn address_book_entry_roundtrip() {
        let entry = AddressBookEntry::new("example.i2p", "base64dest");
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: AddressBookEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, deserialized);
    }

    #[test]
    fn subscription_set_deterministic() {
        let mut subs = SubscriptionSet::new();
        subs.push("http://sub1.example.com".to_string());
        subs.push("http://sub2.example.com".to_string());
        let json1 = serde_json::to_string(&subs).unwrap();
        let json2 = serde_json::to_string(&subs).unwrap();
        assert_eq!(json1, json2);
    }

    #[test]
    fn subscription_set_no_duplicates() {
        let mut subs = SubscriptionSet::new();
        subs.push("http://sub1.example.com".to_string());
        subs.push("http://sub1.example.com".to_string());
        assert_eq!(subs.len(), 1);
    }

    #[test]
    fn subscription_set_remove() {
        let mut subs = SubscriptionSet::new();
        subs.push("http://sub1.example.com".to_string());
        subs.push("http://sub2.example.com".to_string());
        assert!(subs.remove("http://sub1.example.com"));
        assert_eq!(subs.len(), 1);
        assert!(!subs.contains("http://sub1.example.com"));
    }

    #[test]
    fn address_book_config_deterministic() {
        let mut config = AddressBookConfiguration::new();
        config.insert("key2".to_string(), "value2".to_string());
        config.insert("key1".to_string(), "value1".to_string());
        let json1 = serde_json::to_string(&config).unwrap();
        let json2 = serde_json::to_string(&config).unwrap();
        assert_eq!(json1, json2);
        // BTreeMap ensures deterministic key ordering
        let key1_pos = json1.find("key1").unwrap();
        let key2_pos = json1.find("key2").unwrap();
        assert!(key1_pos < key2_pos);
    }

    #[test]
    fn address_book_config_roundtrip() {
        let mut config = AddressBookConfiguration::new();
        config.insert("foo".to_string(), "bar".to_string());
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: AddressBookConfiguration = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn address_book_request_parse_all() {
        let requests = [
            (AddressBookRequest::List, "List"),
            (AddressBookRequest::Lookup, "Lookup"),
            (AddressBookRequest::Add, "Add"),
            (AddressBookRequest::Update, "Update"),
            (AddressBookRequest::Delete, "Delete"),
        ];
        for (expected, input) in &requests {
            let parsed = AddressBookRequest::from_str_exact(input).unwrap();
            assert_eq!(&parsed, expected);
            assert_eq!(parsed.as_str(), *input);
        }
    }

    #[test]
    fn address_book_request_reject_unknown() {
        assert!(AddressBookRequest::from_str_exact("unknown").is_none());
        assert!(AddressBookRequest::from_str_exact("list").is_none());
    }
}
