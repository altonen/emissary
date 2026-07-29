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

//! Proposal 170 AddressBook administrative API handler.
//!
//! Implements the `AddressBook` JSON-RPC method and the Proposal 170
//! RouterInfo address-book selectors.
//!
//! # Invariants
//!
//! - Authentication must precede parsing of expensive destination payloads.
//! - Entry mutation, `SetSubscriptions`, and `SetConfig` are separate methods.
//! - Book type accepts exactly `private`, `local`, `router`, and `published`.
//! - `Delete` selects deletion by parameter presence, not boolean value.
//! - Hostnames are bounded and validated before persistence.
//! - Destinations are decoded and structurally parsed through existing Emissary primitives.
//! - Invalid destinations never reach the store.
//! - Add/update is durable before success is returned.
//! - Delete is durable before success is returned.
//! - Each mutation affects exactly one administrative book.
//! - All four books remain independent across restart.
//! - Listing/lookup follows the exact result shape and deterministic ordering.
//! - Oversize results fail explicitly and are never truncated.
//! - No handler writes `router.toml`.
//! - No handler calls current runtime `AddressBookHandle` mutators.
//! - No administrative state changes runtime destination resolution.
//! - Logs and errors contain no full destination, subscription value, configuration value, token, or state path.

use crate::i2pcontrol::domain::address_book::{
    AddressBookConfiguration, AddressBookEntry, AdministrativeAddressBookType, SubscriptionSet,
};
use crate::i2pcontrol::rpc::{
    self, JsonRpcErrorResponse, JsonRpcRequest, JsonRpcSuccess, RequestId,
};
use crate::i2pcontrol::server::I2pControlState;

const LOG_TARGET: &str = "emissary::i2pcontrol::address_book";

/// Maximum number of entries in a List result.
const MAX_LIST_ENTRIES: usize = 10000;

/// Maximum total byte size of a List result.
const MAX_LIST_BYTES: usize = 4 * 1024 * 1024;

/// Maximum number of subscriptions in a SetSubscriptions request.
const MAX_SUBSCRIPTIONS: usize = 1000;

/// Maximum length of a single subscription URL.
const MAX_SUBSCRIPTION_LENGTH: usize = 2048;

/// Maximum number of configuration entries in a SetConfig request.
const MAX_CONFIG_ENTRIES: usize = 1000;

/// Maximum length of a configuration key.
const MAX_CONFIG_KEY_LENGTH: usize = 256;

/// Maximum length of a configuration value.
const MAX_CONFIG_VALUE_LENGTH: usize = 4096;

/// Maximum length of a hostname.
const MAX_HOSTNAME_LENGTH: usize = 254;

/// AddressBook handler.
pub(crate) async fn handle_address_book(
    state: &I2pControlState,
    request: &JsonRpcRequest,
) -> serde_json::Value {
    let id = resolve_id(&request.id);

    // Parse parameters
    let params = match &request.params {
        Some(params) => params,
        None => {
            return error_response(id, rpc::error_codes::INVALID_PARAMS, "Missing parameters");
        }
    };

    // Extract and validate book type
    let book_type = match params.get("book").and_then(|v| v.as_str()) {
        Some(s) => match AdministrativeAddressBookType::from_str_exact(s) {
            Some(bt) => bt,
            None => {
                return error_response(
                    id,
                    rpc::error_codes::INVALID_PARAMS,
                    format!(
                        "Invalid book type {}; expected one of: private, local, router, published",
                        s
                    ),
                );
            }
        },
        None => {
            return error_response(
                id,
                rpc::error_codes::INVALID_PARAMS,
                "Missing 'book' parameter",
            );
        }
    };

    // Extract request mode
    let request_mode = match params.get("request").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return error_response(
                id,
                rpc::error_codes::INVALID_PARAMS,
                "Missing 'request' parameter",
            );
        }
    };

    match request_mode {
        "List" => handle_list(state, id, book_type).await,
        "Lookup" => handle_lookup(state, id, book_type, params).await,
        "Add" => handle_add(state, id, book_type, params).await,
        "Update" => handle_update(state, id, book_type, params).await,
        "Delete" => handle_delete(state, id, book_type, params).await,
        _ => error_response(
            id,
            rpc::error_codes::INVALID_PARAMS,
            format!(
                "Invalid request mode {}; expected one of: List, Lookup, Add, Update, Delete",
                request_mode
            ),
        ),
    }
}

/// Handle List request: return all entries in the specified book.
async fn handle_list(
    state: &I2pControlState,
    id: RequestId,
    book_type: AdministrativeAddressBookType,
) -> serde_json::Value {
    let entries = match state.address_book_list(book_type).await {
        Ok(entries) => entries,
        Err(e) => {
            tracing::error!(target: LOG_TARGET, "List failed: {}", e);
            return error_response(
                id,
                rpc::error_codes::INTERNAL_ERROR,
                "Failed to list address book entries",
            );
        }
    };

    // Build result array with deterministic ordering (BTreeMap ensures this)
    let result: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "name": e.hostname,
                "value": e.destination,
            })
        })
        .collect();

    // Check size bounds
    let json_size = serde_json::to_string(&result).map(|s| s.len()).unwrap_or(0);
    if result.len() > MAX_LIST_ENTRIES || json_size > MAX_LIST_BYTES {
        return error_response(
            id,
            rpc::error_codes::APP_ERROR,
            "Response too large; reduce the number of entries or use Lookup for specific entries",
        );
    }

    success_response(id, serde_json::json!(result))
}

/// Handle Lookup request: return a specific entry or null.
async fn handle_lookup(
    state: &I2pControlState,
    id: RequestId,
    book_type: AdministrativeAddressBookType,
    params: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let hostname = match params.get("name").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return error_response(
                id,
                rpc::error_codes::INVALID_PARAMS,
                "Missing 'name' parameter for Lookup",
            );
        }
    };

    if let Err(e) = validate_hostname(hostname) {
        return error_response(id, rpc::error_codes::INVALID_PARAMS, e);
    }

    match state.address_book_lookup(book_type, hostname).await {
        Ok(Some(entry)) => success_response(
            id,
            serde_json::json!({
                "name": entry.hostname,
                "value": entry.destination,
            }),
        ),
        Ok(None) => success_response(id, serde_json::Value::Null),
        Err(e) => {
            tracing::error!(target: LOG_TARGET, "Lookup failed: {}", e);
            error_response(
                id,
                rpc::error_codes::INTERNAL_ERROR,
                "Failed to look up address book entry",
            )
        }
    }
}

/// Handle Add request: create a new entry.
async fn handle_add(
    state: &I2pControlState,
    id: RequestId,
    book_type: AdministrativeAddressBookType,
    params: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let hostname = match params.get("name").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return error_response(
                id,
                rpc::error_codes::INVALID_PARAMS,
                "Missing 'name' parameter for Add",
            );
        }
    };

    if let Err(e) = validate_hostname(hostname) {
        return error_response(id, rpc::error_codes::INVALID_PARAMS, e);
    }

    let destination = match params.get("value").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return error_response(
                id,
                rpc::error_codes::INVALID_PARAMS,
                "Missing 'value' parameter for Add",
            );
        }
    };

    if let Err(e) = validate_destination(destination) {
        return error_response(id, rpc::error_codes::INVALID_PARAMS, e);
    }

    let entry = AddressBookEntry::new(hostname, destination);

    match state.address_book_add(book_type, entry).await {
        Ok(()) => success_response(id, serde_json::json!("ok")),
        Err(e) => {
            tracing::error!(target: LOG_TARGET, "Add failed: {}", e);
            error_response(
                id,
                rpc::error_codes::INTERNAL_ERROR,
                "Failed to persist address book entry",
            )
        }
    }
}

/// Handle Update request: update an existing entry.
async fn handle_update(
    state: &I2pControlState,
    id: RequestId,
    book_type: AdministrativeAddressBookType,
    params: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let hostname = match params.get("name").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return error_response(
                id,
                rpc::error_codes::INVALID_PARAMS,
                "Missing 'name' parameter for Update",
            );
        }
    };

    if let Err(e) = validate_hostname(hostname) {
        return error_response(id, rpc::error_codes::INVALID_PARAMS, e);
    }

    let destination = match params.get("value").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return error_response(
                id,
                rpc::error_codes::INVALID_PARAMS,
                "Missing 'value' parameter for Update",
            );
        }
    };

    if let Err(e) = validate_destination(destination) {
        return error_response(id, rpc::error_codes::INVALID_PARAMS, e);
    }

    let entry = AddressBookEntry::new(hostname, destination);

    match state.address_book_update(book_type, entry).await {
        Ok(true) => success_response(id, serde_json::json!("ok")),
        Ok(false) => error_response(
            id,
            rpc::error_codes::APP_ERROR,
            format!("Entry '{}' not found in {} book", hostname, book_type),
        ),
        Err(e) => {
            tracing::error!(target: LOG_TARGET, "Update failed: {}", e);
            error_response(
                id,
                rpc::error_codes::INTERNAL_ERROR,
                "Failed to persist address book entry",
            )
        }
    }
}

/// Handle Delete request: delete an entry or all entries.
///
/// Proposal 170 Delete semantics: presence of `name` parameter selects
/// deletion of a specific entry; absence of `name` deletes all entries
/// in the book.
async fn handle_delete(
    state: &I2pControlState,
    id: RequestId,
    book_type: AdministrativeAddressBookType,
    params: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    // Delete-by-presence: check if `name` is present (any value)
    match params.get("name") {
        Some(name_val) => {
            let hostname = match name_val.as_str() {
                Some(s) => s,
                None => {
                    return error_response(
                        id,
                        rpc::error_codes::INVALID_PARAMS,
                        "'name' parameter must be a string",
                    );
                }
            };

            if let Err(e) = validate_hostname(hostname) {
                return error_response(id, rpc::error_codes::INVALID_PARAMS, e);
            }

            match state.address_book_delete(book_type, hostname).await {
                Ok(true) => success_response(id, serde_json::json!("ok")),
                Ok(false) => success_response(id, serde_json::json!("ok")),
                Err(e) => {
                    tracing::error!(target: LOG_TARGET, "Delete failed: {}", e);
                    error_response(
                        id,
                        rpc::error_codes::INTERNAL_ERROR,
                        "Failed to delete address book entry",
                    )
                }
            }
        }
        None => {
            // No `name` parameter: delete all entries in the book
            match state.address_book_delete_all(book_type).await {
                Ok(true) => success_response(id, serde_json::json!("ok")),
                Ok(false) => success_response(id, serde_json::json!("ok")),
                Err(e) => {
                    tracing::error!(target: LOG_TARGET, "Delete all failed: {}", e);
                    error_response(
                        id,
                        rpc::error_codes::INTERNAL_ERROR,
                        "Failed to delete address book entries",
                    )
                }
            }
        }
    }
}

/// Handle SetSubscriptions method: replace the subscription set.
pub(crate) async fn handle_set_subscriptions(
    state: &I2pControlState,
    request: &JsonRpcRequest,
) -> serde_json::Value {
    let id = resolve_id(&request.id);

    let params = match &request.params {
        Some(params) => params,
        None => {
            return error_response(id, rpc::error_codes::INVALID_PARAMS, "Missing parameters");
        }
    };

    // Extract subscriptions array
    let subs_array = match params.get("subscriptions") {
        Some(serde_json::Value::Array(arr)) => arr,
        _ => {
            return error_response(
                id,
                rpc::error_codes::INVALID_PARAMS,
                "Missing or invalid 'subscriptions' parameter; expected a JSON array",
            );
        }
    };

    // Validate bounds
    if subs_array.len() > MAX_SUBSCRIPTIONS {
        return error_response(
            id,
            rpc::error_codes::INVALID_PARAMS,
            format!("Too many subscriptions; maximum is {}", MAX_SUBSCRIPTIONS),
        );
    }

    let mut subscriptions = SubscriptionSet::new();
    for (i, item) in subs_array.iter().enumerate() {
        let url = match item.as_str() {
            Some(s) => s,
            None => {
                return error_response(
                    id,
                    rpc::error_codes::INVALID_PARAMS,
                    format!("Subscription {} is not a string", i),
                );
            }
        };

        if url.len() > MAX_SUBSCRIPTION_LENGTH {
            return error_response(
                id,
                rpc::error_codes::INVALID_PARAMS,
                format!(
                    "Subscription {} exceeds maximum length of {}",
                    i, MAX_SUBSCRIPTION_LENGTH
                ),
            );
        }

        // Reject control characters
        if url.chars().any(|c| c.is_control()) {
            return error_response(
                id,
                rpc::error_codes::INVALID_PARAMS,
                format!("Subscription {} contains control characters", i),
            );
        }

        subscriptions.push(url.to_string());
    }

    match state.address_book_set_subscriptions(subscriptions).await {
        Ok(()) => success_response(id, serde_json::json!("ok")),
        Err(e) => {
            tracing::error!(target: LOG_TARGET, "SetSubscriptions failed: {}", e);
            error_response(
                id,
                rpc::error_codes::INTERNAL_ERROR,
                "Failed to persist subscriptions",
            )
        }
    }
}

/// Handle SetConfig method: update the address book configuration.
pub(crate) async fn handle_set_config(
    state: &I2pControlState,
    request: &JsonRpcRequest,
) -> serde_json::Value {
    let id = resolve_id(&request.id);

    let params = match &request.params {
        Some(params) => params,
        None => {
            return error_response(id, rpc::error_codes::INVALID_PARAMS, "Missing parameters");
        }
    };

    // Extract config map
    let config_map = match params.get("config") {
        Some(serde_json::Value::Object(map)) => map,
        _ => {
            return error_response(
                id,
                rpc::error_codes::INVALID_PARAMS,
                "Missing or invalid 'config' parameter; expected a JSON object",
            );
        }
    };

    // Validate bounds
    if config_map.len() > MAX_CONFIG_ENTRIES {
        return error_response(
            id,
            rpc::error_codes::INVALID_PARAMS,
            format!(
                "Too many configuration entries; maximum is {}",
                MAX_CONFIG_ENTRIES
            ),
        );
    }

    let mut configuration = AddressBookConfiguration::new();
    for (key, value) in config_map {
        if key.len() > MAX_CONFIG_KEY_LENGTH {
            return error_response(
                id,
                rpc::error_codes::INVALID_PARAMS,
                format!(
                    "Configuration key exceeds maximum length of {}",
                    MAX_CONFIG_KEY_LENGTH
                ),
            );
        }

        let val_str = match value.as_str() {
            Some(s) => s,
            None => {
                return error_response(
                    id,
                    rpc::error_codes::INVALID_PARAMS,
                    format!("Configuration value for '{}' is not a string", key),
                );
            }
        };

        if val_str.len() > MAX_CONFIG_VALUE_LENGTH {
            return error_response(
                id,
                rpc::error_codes::INVALID_PARAMS,
                format!(
                    "Configuration value for '{}' exceeds maximum length of {}",
                    key, MAX_CONFIG_VALUE_LENGTH
                ),
            );
        }

        // Reject control characters in keys and values
        if key.chars().any(|c| c.is_control()) || val_str.chars().any(|c| c.is_control()) {
            return error_response(
                id,
                rpc::error_codes::INVALID_PARAMS,
                format!("Configuration key '{}' contains control characters", key),
            );
        }

        configuration.insert(key.clone(), val_str.to_string());
    }

    match state.address_book_set_configuration(configuration).await {
        Ok(()) => success_response(id, serde_json::json!("ok")),
        Err(e) => {
            tracing::error!(target: LOG_TARGET, "SetConfig failed: {}", e);
            error_response(
                id,
                rpc::error_codes::INTERNAL_ERROR,
                "Failed to persist configuration",
            )
        }
    }
}

/// Validate a hostname for Proposal 170 address book entries.
///
/// Hostnames must be non-empty, at most `MAX_HOSTNAME_LENGTH` bytes,
/// and must not contain path separators, NULs, or control characters.
fn validate_hostname(hostname: &str) -> Result<(), String> {
    if hostname.is_empty() {
        return Err("Hostname must not be empty".to_string());
    }

    if hostname.len() > MAX_HOSTNAME_LENGTH {
        return Err(format!(
            "Hostname exceeds maximum length of {}",
            MAX_HOSTNAME_LENGTH
        ));
    }

    if hostname.contains('\0') {
        return Err("Hostname must not contain NUL characters".to_string());
    }

    if hostname.contains('/') || hostname.contains('\\') {
        return Err("Hostname must not contain path separators".to_string());
    }

    if hostname.chars().any(|c| c.is_control()) {
        return Err("Hostname must not contain control characters".to_string());
    }

    Ok(())
}

/// Validate a destination for Proposal 170 address book entries.
///
/// Destinations must be non-empty and must not contain control characters.
/// Full I2P destination parsing is performed by the destination decoder.
fn validate_destination(destination: &str) -> Result<(), String> {
    if destination.is_empty() {
        return Err("Destination must not be empty".to_string());
    }

    if destination.len() > 1024 * 1024 {
        return Err("Destination exceeds maximum length".to_string());
    }

    if destination.chars().any(|c| c.is_control()) {
        return Err("Destination must not contain control characters".to_string());
    }

    Ok(())
}

fn resolve_id(id: &Option<RequestId>) -> RequestId {
    id.clone().unwrap_or(RequestId::Null)
}

fn success_response(id: RequestId, result: serde_json::Value) -> serde_json::Value {
    serde_json::to_value(JsonRpcSuccess::new(id, result)).unwrap()
}

fn error_response(id: RequestId, code: i32, message: impl Into<String>) -> serde_json::Value {
    serde_json::to_value(JsonRpcErrorResponse::new(id, code, message)).unwrap()
}

/// RouterInfo address-book selector adapter.
///
/// Given a set of requested selector keys and an address-book control plane,
/// returns a JSON object containing only the requested address-book fields.
/// This function is called by the M005 RouterInfo handler to provide
/// address-book data from the administrative stores.
///
/// # Selector keys
///
/// - `i2p.router.addressbook.private` — array of private book entries
/// - `i2p.router.addressbook.local` — array of local book entries
/// - `i2p.router.addressbook.router` — array of router book entries
/// - `i2p.router.addressbook.published` — array of published book entries
/// - `i2p.router.addressbook.subscriptions` — array of subscription URLs
/// - `i2p.router.addressbook.config` — configuration key-value object
///
/// # Response format
///
/// Each selector returns a JSON array or object. Only requested selectors
/// appear in the result. Entries are ordered deterministically by hostname.
pub async fn resolve_address_book_selectors(
    control: &dyn crate::i2pcontrol::control_plane::AddressBookControl,
    requested_keys: &[&str],
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let mut result = serde_json::Map::new();

    for key in requested_keys {
        match *key {
            rpc::router_info_keys::ADDRESS_BOOK_PRIVATE => {
                let entries = control.list(AdministrativeAddressBookType::Private).await?;
                result.insert(
                    key.to_string(),
                    serde_json::json!(entries_to_json(&entries)),
                );
            }
            rpc::router_info_keys::ADDRESS_BOOK_LOCAL => {
                let entries = control.list(AdministrativeAddressBookType::Local).await?;
                result.insert(
                    key.to_string(),
                    serde_json::json!(entries_to_json(&entries)),
                );
            }
            rpc::router_info_keys::ADDRESS_BOOK_ROUTER => {
                let entries = control.list(AdministrativeAddressBookType::Router).await?;
                result.insert(
                    key.to_string(),
                    serde_json::json!(entries_to_json(&entries)),
                );
            }
            rpc::router_info_keys::ADDRESS_BOOK_PUBLISHED => {
                let entries = control.list(AdministrativeAddressBookType::Published).await?;
                result.insert(
                    key.to_string(),
                    serde_json::json!(entries_to_json(&entries)),
                );
            }
            rpc::router_info_keys::ADDRESS_BOOK_SUBSCRIPTIONS => {
                let subs = control.subscriptions().await?;
                let urls: Vec<&str> = subs.as_slice().iter().map(|s| s.as_str()).collect();
                result.insert(key.to_string(), serde_json::json!(urls));
            }
            rpc::router_info_keys::ADDRESS_BOOK_CONFIG => {
                let config = control.configuration().await?;
                let map: serde_json::Map<String, serde_json::Value> = config
                    .as_map()
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::json!(v)))
                    .collect();
                result.insert(key.to_string(), serde_json::json!(map));
            }
            _ => {
                // Unknown selector — skip (not an error, just not our key)
            }
        }
    }

    Ok(result)
}

/// Convert address book entries to the Proposal 170 JSON format.
fn entries_to_json(entries: &[AddressBookEntry]) -> Vec<serde_json::Value> {
    entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "name": e.hostname,
                "value": e.destination,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_hostname_valid() {
        assert!(validate_hostname("example.i2p").is_ok());
        assert!(validate_hostname("a.b.c.i2p").is_ok());
        assert!(validate_hostname("test").is_ok());
    }

    #[test]
    fn validate_hostname_empty() {
        assert!(validate_hostname("").is_err());
    }

    #[test]
    fn validate_hostname_too_long() {
        let long = "a".repeat(MAX_HOSTNAME_LENGTH + 1);
        assert!(validate_hostname(&long).is_err());
    }

    #[test]
    fn validate_hostname_nul() {
        assert!(validate_hostname("test\0.i2p").is_err());
    }

    #[test]
    fn validate_hostname_path_separator() {
        assert!(validate_hostname("test/foo.i2p").is_err());
        assert!(validate_hostname("test\\foo.i2p").is_err());
    }

    #[test]
    fn validate_hostname_control_chars() {
        assert!(validate_hostname("test\n.i2p").is_err());
        assert!(validate_hostname("test\t.i2p").is_err());
    }

    #[test]
    fn validate_destination_valid() {
        assert!(validate_destination("base64dest").is_ok());
        assert!(validate_destination(&"A".repeat(1000)).is_ok());
    }

    #[test]
    fn validate_destination_empty() {
        assert!(validate_destination("").is_err());
    }

    #[test]
    fn validate_destination_too_long() {
        let long = "A".repeat(1024 * 1024 + 1);
        assert!(validate_destination(&long).is_err());
    }

    #[test]
    fn validate_destination_control_chars() {
        assert!(validate_destination("test\n").is_err());
    }

    // --- Handler integration tests using FakeAddressBookControl ---

    use crate::i2pcontrol::control_plane::{AddressBookControl, FakeAddressBookControl};
    use crate::i2pcontrol::rpc::JsonRpcRequest;

    fn test_state() -> crate::i2pcontrol::server::I2pControlState {
        let mut state =
            crate::i2pcontrol::server::I2pControlState::new_test("testpass".to_string());
        // Replace the address book control with a fresh fake
        state.set_address_book_control(Box::new(FakeAddressBookControl::new()));
        state
    }

    fn ab_request(method: &str, params: serde_json::Value) -> JsonRpcRequest {
        let params_map = params.as_object().cloned().unwrap();
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params: Some(params_map),
            id: Some(crate::i2pcontrol::rpc::RequestId::Number(1)),
        }
    }

    #[tokio::test]
    async fn handler_list_empty_book() {
        let state = test_state();
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "private", "request": "List"}),
        );
        let resp = handle_address_book(&state, &req).await;
        assert_eq!(resp["jsonrpc"], "2.0");
        assert!(resp["result"].is_array());
        assert_eq!(resp["result"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn handler_add_and_list() {
        let state = test_state();
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "local", "request": "Add", "name": "test.i2p", "value": "base64dest"}),
        );
        let resp = handle_address_book(&state, &req).await;
        assert_eq!(resp["result"], "ok");

        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "local", "request": "List"}),
        );
        let resp = handle_address_book(&state, &req).await;
        let arr = resp["result"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["name"], "test.i2p");
        assert_eq!(arr[0]["value"], "base64dest");
    }

    #[tokio::test]
    async fn handler_lookup_found() {
        let state = test_state();
        // Add first
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "router", "request": "Add", "name": "found.i2p", "value": "dest123"}),
        );
        handle_address_book(&state, &req).await;

        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "router", "request": "Lookup", "name": "found.i2p"}),
        );
        let resp = handle_address_book(&state, &req).await;
        assert_eq!(resp["result"]["name"], "found.i2p");
        assert_eq!(resp["result"]["value"], "dest123");
    }

    #[tokio::test]
    async fn handler_lookup_not_found() {
        let state = test_state();
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "published", "request": "Lookup", "name": "missing.i2p"}),
        );
        let resp = handle_address_book(&state, &req).await;
        assert!(resp["result"].is_null());
    }

    #[tokio::test]
    async fn handler_update_existing() {
        let state = test_state();
        // Add
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "private", "request": "Add", "name": "host.i2p", "value": "old"}),
        );
        handle_address_book(&state, &req).await;

        // Update
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "private", "request": "Update", "name": "host.i2p", "value": "new"}),
        );
        let resp = handle_address_book(&state, &req).await;
        assert_eq!(resp["result"], "ok");

        // Verify
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "private", "request": "Lookup", "name": "host.i2p"}),
        );
        let resp = handle_address_book(&state, &req).await;
        assert_eq!(resp["result"]["value"], "new");
    }

    #[tokio::test]
    async fn handler_update_not_found() {
        let state = test_state();
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "private", "request": "Update", "name": "noexist.i2p", "value": "dest"}),
        );
        let resp = handle_address_book(&state, &req).await;
        assert_eq!(resp["error"]["code"], -1);
    }

    #[tokio::test]
    async fn handler_delete_by_name() {
        let state = test_state();
        // Add
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "local", "request": "Add", "name": "del.i2p", "value": "dest"}),
        );
        handle_address_book(&state, &req).await;

        // Delete with name present
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "local", "request": "Delete", "name": "del.i2p"}),
        );
        let resp = handle_address_book(&state, &req).await;
        assert_eq!(resp["result"], "ok");

        // Verify deleted
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "local", "request": "Lookup", "name": "del.i2p"}),
        );
        let resp = handle_address_book(&state, &req).await;
        assert!(resp["result"].is_null());
    }

    #[tokio::test]
    async fn handler_delete_all() {
        let state = test_state();
        // Add entries
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "router", "request": "Add", "name": "a.i2p", "value": "dest-a"}),
        );
        handle_address_book(&state, &req).await;
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "router", "request": "Add", "name": "b.i2p", "value": "dest-b"}),
        );
        handle_address_book(&state, &req).await;

        // Delete all (no name parameter)
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "router", "request": "Delete"}),
        );
        let resp = handle_address_book(&state, &req).await;
        assert_eq!(resp["result"], "ok");

        // Verify empty
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "router", "request": "List"}),
        );
        let resp = handle_address_book(&state, &req).await;
        assert_eq!(resp["result"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn handler_invalid_book_type() {
        let state = test_state();
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "invalid", "request": "List"}),
        );
        let resp = handle_address_book(&state, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn handler_missing_book_param() {
        let state = test_state();
        let req = ab_request("AddressBook", serde_json::json!({"request": "List"}));
        let resp = handle_address_book(&state, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn handler_missing_request_param() {
        let state = test_state();
        let req = ab_request("AddressBook", serde_json::json!({"book": "private"}));
        let resp = handle_address_book(&state, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn handler_invalid_request_mode() {
        let state = test_state();
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "private", "request": "Invalid"}),
        );
        let resp = handle_address_book(&state, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn handler_add_missing_name() {
        let state = test_state();
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "private", "request": "Add", "value": "dest"}),
        );
        let resp = handle_address_book(&state, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn handler_add_missing_value() {
        let state = test_state();
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "private", "request": "Add", "name": "test.i2p"}),
        );
        let resp = handle_address_book(&state, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn handler_add_invalid_hostname() {
        let state = test_state();
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "private", "request": "Add", "name": "", "value": "dest"}),
        );
        let resp = handle_address_book(&state, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn handler_add_invalid_destination() {
        let state = test_state();
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "private", "request": "Add", "name": "test.i2p", "value": ""}),
        );
        let resp = handle_address_book(&state, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn handler_no_params() {
        let state = test_state();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "AddressBook".to_string(),
            params: None,
            id: Some(crate::i2pcontrol::rpc::RequestId::Number(1)),
        };
        let resp = handle_address_book(&state, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn handler_delete_not_found() {
        let state = test_state();
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "private", "request": "Delete", "name": "noexist.i2p"}),
        );
        let resp = handle_address_book(&state, &req).await;
        // Delete of absent entry is a successful no-op
        assert_eq!(resp["result"], "ok");
    }

    // --- SetSubscriptions handler tests ---

    #[tokio::test]
    async fn handler_set_subscriptions_success() {
        let state = test_state();
        let req = ab_request(
            "SetSubscriptions",
            serde_json::json!({"subscriptions": ["http://sub1.example.com", "http://sub2.example.com"]}),
        );
        let resp = handle_set_subscriptions(&state, &req).await;
        assert_eq!(resp["result"], "ok");
    }

    #[tokio::test]
    async fn handler_set_subscriptions_dedup() {
        let state = test_state();
        let req = ab_request(
            "SetSubscriptions",
            serde_json::json!({"subscriptions": ["http://sub.example.com", "http://sub.example.com"]}),
        );
        let resp = handle_set_subscriptions(&state, &req).await;
        assert_eq!(resp["result"], "ok");

        // Verify dedup
        let subs = state.address_book_subscriptions().await.unwrap();
        assert_eq!(subs.len(), 1);
    }

    #[tokio::test]
    async fn handler_set_subscriptions_empty() {
        let state = test_state();
        let req = ab_request("SetSubscriptions", serde_json::json!({"subscriptions": []}));
        let resp = handle_set_subscriptions(&state, &req).await;
        assert_eq!(resp["result"], "ok");
    }

    #[tokio::test]
    async fn handler_set_subscriptions_missing_param() {
        let state = test_state();
        let req = ab_request("SetSubscriptions", serde_json::json!({}));
        let resp = handle_set_subscriptions(&state, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn handler_set_subscriptions_non_string_item() {
        let state = test_state();
        let req = ab_request(
            "SetSubscriptions",
            serde_json::json!({"subscriptions": [123]}),
        );
        let resp = handle_set_subscriptions(&state, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn handler_set_subscriptions_control_chars() {
        let state = test_state();
        let req = ab_request(
            "SetSubscriptions",
            serde_json::json!({"subscriptions": ["http://sub.example.com\n"]}),
        );
        let resp = handle_set_subscriptions(&state, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    // --- SetConfig handler tests ---

    #[tokio::test]
    async fn handler_set_config_success() {
        let state = test_state();
        let req = ab_request(
            "SetConfig",
            serde_json::json!({"config": {"mode": "aggressive", "level": "3"}}),
        );
        let resp = handle_set_config(&state, &req).await;
        assert_eq!(resp["result"], "ok");
    }

    #[tokio::test]
    async fn handler_set_config_empty() {
        let state = test_state();
        let req = ab_request("SetConfig", serde_json::json!({"config": {}}));
        let resp = handle_set_config(&state, &req).await;
        assert_eq!(resp["result"], "ok");
    }

    #[tokio::test]
    async fn handler_set_config_missing_param() {
        let state = test_state();
        let req = ab_request("SetConfig", serde_json::json!({}));
        let resp = handle_set_config(&state, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn handler_set_config_non_string_value() {
        let state = test_state();
        let req = ab_request("SetConfig", serde_json::json!({"config": {"key": 123}}));
        let resp = handle_set_config(&state, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn handler_set_config_control_chars() {
        let state = test_state();
        let req = ab_request(
            "SetConfig",
            serde_json::json!({"config": {"key\n": "value"}}),
        );
        let resp = handle_set_config(&state, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    // --- Book isolation tests ---

    #[tokio::test]
    async fn handler_book_isolation() {
        let state = test_state();

        // Add to private
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "private", "request": "Add", "name": "p.i2p", "value": "p-dest"}),
        );
        handle_address_book(&state, &req).await;

        // Add to local
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "local", "request": "Add", "name": "l.i2p", "value": "l-dest"}),
        );
        handle_address_book(&state, &req).await;

        // List private - only 1 entry
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "private", "request": "List"}),
        );
        let resp = handle_address_book(&state, &req).await;
        assert_eq!(resp["result"].as_array().unwrap().len(), 1);

        // List local - only 1 entry
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "local", "request": "List"}),
        );
        let resp = handle_address_book(&state, &req).await;
        assert_eq!(resp["result"].as_array().unwrap().len(), 1);

        // List router - empty
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "router", "request": "List"}),
        );
        let resp = handle_address_book(&state, &req).await;
        assert_eq!(resp["result"].as_array().unwrap().len(), 0);

        // List published - empty
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "published", "request": "List"}),
        );
        let resp = handle_address_book(&state, &req).await;
        assert_eq!(resp["result"].as_array().unwrap().len(), 0);
    }

    // --- Delete-by-presence semantics test ---

    #[tokio::test]
    async fn handler_delete_presence_with_false_value() {
        let state = test_state();
        // Add entry
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "private", "request": "Add", "name": "host.i2p", "value": "dest"}),
        );
        handle_address_book(&state, &req).await;

        // Delete with name present (even if value is not "true")
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "private", "request": "Delete", "name": "host.i2p"}),
        );
        let resp = handle_address_book(&state, &req).await;
        assert_eq!(resp["result"], "ok");

        // Verify deleted
        let req = ab_request(
            "AddressBook",
            serde_json::json!({"book": "private", "request": "Lookup", "name": "host.i2p"}),
        );
        let resp = handle_address_book(&state, &req).await;
        assert!(resp["result"].is_null());
    }

    // --- RouterInfo address-book selector tests ---

    #[tokio::test]
    async fn selector_private_book_empty() {
        let cp = FakeAddressBookControl::new();
        let result =
            resolve_address_book_selectors(&cp, &[rpc::router_info_keys::ADDRESS_BOOK_PRIVATE])
                .await
                .unwrap();
        assert_eq!(
            result.get(rpc::router_info_keys::ADDRESS_BOOK_PRIVATE).unwrap(),
            &serde_json::json!([])
        );
    }

    #[tokio::test]
    async fn selector_local_book_with_entries() {
        let cp = FakeAddressBookControl::new();
        cp.add(
            AdministrativeAddressBookType::Local,
            AddressBookEntry::new("test.i2p", "dest"),
        )
        .await
        .unwrap();

        let result =
            resolve_address_book_selectors(&cp, &[rpc::router_info_keys::ADDRESS_BOOK_LOCAL])
                .await
                .unwrap();
        let arr = result
            .get(rpc::router_info_keys::ADDRESS_BOOK_LOCAL)
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["name"], "test.i2p");
        assert_eq!(arr[0]["value"], "dest");
    }

    #[tokio::test]
    async fn selector_router_book_empty() {
        let cp = FakeAddressBookControl::new();
        let result =
            resolve_address_book_selectors(&cp, &[rpc::router_info_keys::ADDRESS_BOOK_ROUTER])
                .await
                .unwrap();
        assert_eq!(
            result.get(rpc::router_info_keys::ADDRESS_BOOK_ROUTER).unwrap(),
            &serde_json::json!([])
        );
    }

    #[tokio::test]
    async fn selector_published_book_empty() {
        let cp = FakeAddressBookControl::new();
        let result =
            resolve_address_book_selectors(&cp, &[rpc::router_info_keys::ADDRESS_BOOK_PUBLISHED])
                .await
                .unwrap();
        assert_eq!(
            result.get(rpc::router_info_keys::ADDRESS_BOOK_PUBLISHED).unwrap(),
            &serde_json::json!([])
        );
    }

    #[tokio::test]
    async fn selector_subscriptions() {
        let cp = FakeAddressBookControl::new();
        let mut subs = SubscriptionSet::new();
        subs.push("http://sub1.example.com".to_string());
        subs.push("http://sub2.example.com".to_string());
        cp.set_subscriptions(subs).await.unwrap();

        let result = resolve_address_book_selectors(
            &cp,
            &[rpc::router_info_keys::ADDRESS_BOOK_SUBSCRIPTIONS],
        )
        .await
        .unwrap();
        let arr = result
            .get(rpc::router_info_keys::ADDRESS_BOOK_SUBSCRIPTIONS)
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0], "http://sub1.example.com");
        assert_eq!(arr[1], "http://sub2.example.com");
    }

    #[tokio::test]
    async fn selector_config() {
        let cp = FakeAddressBookControl::new();
        let mut config = AddressBookConfiguration::new();
        config.insert("mode".to_string(), "aggressive".to_string());
        cp.set_configuration(config).await.unwrap();

        let result =
            resolve_address_book_selectors(&cp, &[rpc::router_info_keys::ADDRESS_BOOK_CONFIG])
                .await
                .unwrap();
        let obj = result
            .get(rpc::router_info_keys::ADDRESS_BOOK_CONFIG)
            .unwrap()
            .as_object()
            .unwrap();
        assert_eq!(obj.get("mode").unwrap(), "aggressive");
    }

    #[tokio::test]
    async fn selector_multiple_keys() {
        let cp = FakeAddressBookControl::new();
        cp.add(
            AdministrativeAddressBookType::Private,
            AddressBookEntry::new("p.i2p", "p-dest"),
        )
        .await
        .unwrap();
        let mut subs = SubscriptionSet::new();
        subs.push("http://sub.example.com".to_string());
        cp.set_subscriptions(subs).await.unwrap();

        let result = resolve_address_book_selectors(
            &cp,
            &[
                rpc::router_info_keys::ADDRESS_BOOK_PRIVATE,
                rpc::router_info_keys::ADDRESS_BOOK_SUBSCRIPTIONS,
            ],
        )
        .await
        .unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.contains_key(rpc::router_info_keys::ADDRESS_BOOK_PRIVATE));
        assert!(result.contains_key(rpc::router_info_keys::ADDRESS_BOOK_SUBSCRIPTIONS));
    }

    #[tokio::test]
    async fn selector_unknown_key_ignored() {
        let cp = FakeAddressBookControl::new();
        let result = resolve_address_book_selectors(&cp, &["unknown.selector.key"]).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn selector_empty_request() {
        let cp = FakeAddressBookControl::new();
        let result = resolve_address_book_selectors(&cp, &[]).await.unwrap();
        assert!(result.is_empty());
    }
}
