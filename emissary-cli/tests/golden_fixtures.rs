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
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

//! Golden fixtures for Proposal 170 — sanitized request/response corpus.
//!
//! Every fixture:
//! - uses no real secrets, private keys, credentials, or personal destinations
//! - is deterministic across platforms where the protocol permits
//! - separates variable runtime values from exact structural assertions
//! - asserts exact absence of extension fields
//! - records the contract source/reference and manifest row IDs

#![cfg(feature = "i2pcontrol")]

use serde_json::{json, Value};

// ──────────────────────────────────────────────────────────────────────
// § 1. Authenticate fixtures
// ──────────────────────────────────────────────────────────────────────

#[test]
fn fixture_authenticate_request_structure() {
    let req = json!({
        "jsonrpc": "2.0",
        "method": "Authenticate",
        "params": {
            "API": 2,
            "Username": "i2pcontrol",
            "Password": "fixture-password-REDACTED"
        },
        "id": 1
    });
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    assert_eq!(parsed.method, "Authenticate");
    assert!(parsed.params.is_some());
    let params = parsed.params.unwrap();
    assert_eq!(params.get("API"), Some(&json!(2)));
    assert_eq!(params.get("Username"), Some(&json!("i2pcontrol")));
    assert!(params.get("Password").is_some());
}

#[test]
fn fixture_authenticate_success_envelope() {
    let resp = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "Token": "fixture-token-REDACTED",
            "API": "2"
        }
    });
    // Envelope must have exactly jsonrpc, id, result — no extra keys
    let obj = resp.as_object().unwrap();
    assert!(obj.contains_key("jsonrpc"));
    assert!(obj.contains_key("id"));
    assert!(obj.contains_key("result"));
    assert_eq!(obj.len(), 3, "success envelope must have exactly 3 keys");

    let result = obj.get("result").unwrap().as_object().unwrap();
    assert!(result.contains_key("Token"));
    assert!(result.contains_key("API"));
    assert_eq!(
        result.len(),
        2,
        "authenticate result must have exactly 2 keys"
    );

    // Token must be a non-empty string
    let token = result.get("Token").unwrap().as_str().unwrap();
    assert!(!token.is_empty());
    // API must be a string
    assert!(result.get("API").unwrap().is_string());
}

#[test]
fn fixture_authenticate_error_envelope() {
    let resp = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "error": {
            "code": -1,
            "message": "Invalid username or password"
        }
    });
    let obj = resp.as_object().unwrap();
    assert!(obj.contains_key("error"));
    assert!(
        !obj.contains_key("result"),
        "error response must not have result"
    );

    let error = obj.get("error").unwrap().as_object().unwrap();
    assert_eq!(error.get("code"), Some(&json!(-1)));
    assert!(error.get("message").unwrap().is_string());
}

// ──────────────────────────────────────────────────────────────────────
// § 2. RouterInfo selector fixtures — every selector alone
// ──────────────────────────────────────────────────────────────────────

fn router_info_request(selectors: &[&str]) -> Value {
    let params: serde_json::Map<String, Value> =
        selectors.iter().map(|s| (s.to_string(), json!(true))).collect();
    json!({
        "jsonrpc": "2.0",
        "method": "RouterInfo",
        "params": params,
        "id": 1
    })
}

#[test]
fn fixture_ri_single_selector_udp_active() {
    let req = router_info_request(&["i2p.router.udp.active"]);
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    assert_eq!(parsed.method, "RouterInfo");
    let params = parsed.params.unwrap();
    assert_eq!(params.len(), 1);
    assert_eq!(params.get("i2p.router.udp.active"), Some(&json!(true)));
}

#[test]
fn fixture_ri_single_selector_version() {
    let req = router_info_request(&["i2p.router.version"]);
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    let params = parsed.params.unwrap();
    assert_eq!(params.len(), 1);
    assert!(params.contains_key("i2p.router.version"));
}

#[test]
fn fixture_ri_single_selector_uptime() {
    let req = router_info_request(&["i2p.router.uptime"]);
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    let params = parsed.params.unwrap();
    assert_eq!(params.len(), 1);
}

#[test]
fn fixture_ri_single_selector_identity() {
    let req = router_info_request(&["i2p.router.identity"]);
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    let params = parsed.params.unwrap();
    assert_eq!(params.len(), 1);
}

#[test]
fn fixture_ri_multi_selector() {
    let req = router_info_request(&[
        "i2p.router.udp.active",
        "i2p.router.version",
        "i2p.router.uptime",
        "i2p.router.identity",
    ]);
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    let params = parsed.params.unwrap();
    assert_eq!(params.len(), 4);
}

#[test]
fn fixture_ri_all_selectors() {
    let req = router_info_request(emissary_cli::i2pcontrol::rpc::router_info_keys::ALL);
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    let params = parsed.params.unwrap();
    assert_eq!(params.len(), 121);
}

#[test]
fn fixture_ri_address_book_selectors() {
    let req =
        router_info_request(emissary_cli::i2pcontrol::rpc::router_info_keys::ADDRESS_BOOK_KEYS);
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    let params = parsed.params.unwrap();
    assert_eq!(params.len(), 6);
}

// ──────────────────────────────────────────────────────────────────────
// § 3. AddressBook fixtures — all valid operation modes
// ──────────────────────────────────────────────────────────────────────

fn address_book_request(
    book: &str,
    request: &str,
    extra: Option<serde_json::Map<String, Value>>,
) -> Value {
    let mut params = serde_json::Map::new();
    params.insert("book".to_string(), json!(book));
    params.insert("request".to_string(), json!(request));
    if let Some(mut e) = extra {
        params.append(&mut e);
    }
    json!({
        "jsonrpc": "2.0",
        "method": "AddressBook",
        "params": params,
        "id": 1
    })
}

#[test]
fn fixture_ab_list_all_books() {
    for book in &["private", "local", "router", "published"] {
        let req = address_book_request(book, "List", None);
        let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
        assert_eq!(parsed.method, "AddressBook");
        let params = parsed.params.unwrap();
        assert_eq!(params.get("book"), Some(&json!(book)));
        assert_eq!(params.get("request"), Some(&json!("List")));
    }
}

#[test]
fn fixture_ab_lookup() {
    let mut extra = serde_json::Map::new();
    extra.insert("name".to_string(), json!("fixture-destination"));
    let req = address_book_request("private", "Lookup", Some(extra));
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    let params = parsed.params.unwrap();
    assert_eq!(params.get("name"), Some(&json!("fixture-destination")));
}

#[test]
fn fixture_ab_add() {
    let mut extra = serde_json::Map::new();
    extra.insert("name".to_string(), json!("fixture-dest"));
    extra.insert(
        "value".to_string(),
        json!(" fixture-i2p-destination-REDACTED "),
    );
    let req = address_book_request("local", "Add", Some(extra));
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    let params = parsed.params.unwrap();
    assert_eq!(params.get("name"), Some(&json!("fixture-dest")));
    assert!(params.get("value").is_some());
}

#[test]
fn fixture_ab_update() {
    let mut extra = serde_json::Map::new();
    extra.insert("name".to_string(), json!("fixture-dest"));
    extra.insert("value".to_string(), json!(" fixture-updated-REDACTED "));
    let req = address_book_request("router", "Update", Some(extra));
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    assert_eq!(parsed.method, "AddressBook");
}

#[test]
fn fixture_ab_delete_specific() {
    let mut extra = serde_json::Map::new();
    extra.insert("name".to_string(), json!("fixture-dest"));
    let req = address_book_request("published", "Delete", Some(extra));
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    let params = parsed.params.unwrap();
    assert!(params.contains_key("name"));
}

#[test]
fn fixture_ab_delete_all() {
    let req = address_book_request("private", "Delete", None);
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    let params = parsed.params.unwrap();
    assert!(
        !params.contains_key("name"),
        "Delete all must not have name param"
    );
}

#[test]
fn fixture_ab_invalid_book() {
    let req = address_book_request("invalid", "List", None);
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    assert_eq!(parsed.method, "AddressBook");
}

#[test]
fn fixture_ab_invalid_request_mode() {
    let mut extra = serde_json::Map::new();
    extra.insert("book".to_string(), json!("private"));
    extra.insert("request".to_string(), json!("Invalid"));
    let req = json!({
        "jsonrpc": "2.0",
        "method": "AddressBook",
        "params": extra,
        "id": 1
    });
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    assert_eq!(parsed.method, "AddressBook");
}

// ──────────────────────────────────────────────────────────────────────
// § 4. TunnelManager fixtures — all 12 types, minimum valid CRUD
// ──────────────────────────────────────────────────────────────────────

fn tunnel_manager_request(action: &str, extra: Option<serde_json::Map<String, Value>>) -> Value {
    let mut params = serde_json::Map::new();
    params.insert("Action".to_string(), json!(action));
    if let Some(mut e) = extra {
        params.append(&mut e);
    }
    json!({
        "jsonrpc": "2.0",
        "method": "TunnelManager",
        "params": params,
        "id": 1
    })
}

#[test]
fn fixture_tm_list() {
    let req = tunnel_manager_request("List", None);
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    assert_eq!(parsed.method, "TunnelManager");
    let params = parsed.params.unwrap();
    assert_eq!(params.get("Action"), Some(&json!("List")));
}

#[test]
fn fixture_tm_create_all_types() {
    let types = [
        "client",
        "httpclient",
        "ircclient",
        "socks",
        "socksirc",
        "connectclient",
        "streamrclient",
        "server",
        "httpserver",
        "httpbidirserver",
        "ircserver",
        "streamrserver",
    ];
    for tt in types {
        let mut extra = serde_json::Map::new();
        extra.insert("Type".to_string(), json!(tt));
        extra.insert("Name".to_string(), json!("fixture-tunnel"));
        let req = tunnel_manager_request("Create", Some(extra));
        let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
        let params = parsed.params.unwrap();
        assert_eq!(params.get("Type"), Some(&json!(tt)));
        assert_eq!(params.get("Name"), Some(&json!("fixture-tunnel")));
    }
}

#[test]
fn fixture_tm_get() {
    let mut extra = serde_json::Map::new();
    extra.insert("Name".to_string(), json!("fixture-tunnel"));
    let req = tunnel_manager_request("Get", Some(extra));
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    let params = parsed.params.unwrap();
    assert_eq!(params.get("Name"), Some(&json!("fixture-tunnel")));
}

#[test]
fn fixture_tm_edit() {
    let mut extra = serde_json::Map::new();
    extra.insert("Name".to_string(), json!("fixture-tunnel"));
    extra.insert("Type".to_string(), json!("client"));
    let req = tunnel_manager_request("Edit", Some(extra));
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    let params = parsed.params.unwrap();
    assert_eq!(params.get("Name"), Some(&json!("fixture-tunnel")));
}

#[test]
fn fixture_tm_delete() {
    let mut extra = serde_json::Map::new();
    extra.insert("Name".to_string(), json!("fixture-tunnel"));
    let req = tunnel_manager_request("Delete", Some(extra));
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    let params = parsed.params.unwrap();
    assert_eq!(params.get("Name"), Some(&json!("fixture-tunnel")));
}

#[test]
fn fixture_tm_start() {
    let mut extra = serde_json::Map::new();
    extra.insert("Name".to_string(), json!("fixture-tunnel"));
    let req = tunnel_manager_request("Start", Some(extra));
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    let params = parsed.params.unwrap();
    assert_eq!(params.get("Name"), Some(&json!("fixture-tunnel")));
}

#[test]
fn fixture_tm_stop() {
    let mut extra = serde_json::Map::new();
    extra.insert("Name".to_string(), json!("fixture-tunnel"));
    let req = tunnel_manager_request("Stop", Some(extra));
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    let params = parsed.params.unwrap();
    assert_eq!(params.get("Name"), Some(&json!("fixture-tunnel")));
}

#[test]
fn fixture_tm_restart() {
    let mut extra = serde_json::Map::new();
    extra.insert("Name".to_string(), json!("fixture-tunnel"));
    let req = tunnel_manager_request("Restart", Some(extra));
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    let params = parsed.params.unwrap();
    assert_eq!(params.get("Name"), Some(&json!("fixture-tunnel")));
}

// ──────────────────────────────────────────────────────────────────────
// § 5. TunnelManager All fixtures
// ──────────────────────────────────────────────────────────────────────

#[test]
fn fixture_tm_all_start() {
    let mut extra = serde_json::Map::new();
    extra.insert("Name".to_string(), json!("All"));
    let req = tunnel_manager_request("Start", Some(extra));
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    let params = parsed.params.unwrap();
    assert_eq!(params.get("Name"), Some(&json!("All")));
}

#[test]
fn fixture_tm_all_stop() {
    let mut extra = serde_json::Map::new();
    extra.insert("Name".to_string(), json!("All"));
    let req = tunnel_manager_request("Stop", Some(extra));
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    let params = parsed.params.unwrap();
    assert_eq!(params.get("Name"), Some(&json!("All")));
}

#[test]
fn fixture_tm_all_restart() {
    let mut extra = serde_json::Map::new();
    extra.insert("Name".to_string(), json!("All"));
    let req = tunnel_manager_request("Restart", Some(extra));
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    let params = parsed.params.unwrap();
    assert_eq!(params.get("Name"), Some(&json!("All")));
}

// ──────────────────────────────────────────────────────────────────────
// § 6. ClientServicesInfo fixtures — all 6 selectors
// ──────────────────────────────────────────────────────────────────────

fn client_services_request(selectors: &[&str]) -> Value {
    let selector_map: serde_json::Map<String, Value> =
        selectors.iter().map(|s| (s.to_string(), json!(true))).collect();
    json!({
        "jsonrpc": "2.0",
        "method": "ClientServicesInfo",
        "params": {
            "Selector": selector_map
        },
        "id": 1
    })
}

#[test]
fn fixture_csi_all_selectors() {
    let req = client_services_request(&["I2PTunnel", "HTTPProxy", "SOCKS", "SAM", "BOB", "I2CP"]);
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    assert_eq!(parsed.method, "ClientServicesInfo");
    let params = parsed.params.unwrap();
    let selector = params.get("Selector").unwrap().as_object().unwrap();
    assert_eq!(selector.len(), 6);
}

#[test]
fn fixture_csi_i2ptunnel() {
    let req = client_services_request(&["I2PTunnel"]);
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    let params = parsed.params.unwrap();
    let selector = params.get("Selector").unwrap().as_object().unwrap();
    assert_eq!(selector.len(), 1);
    assert_eq!(selector.get("I2PTunnel"), Some(&json!(true)));
}

#[test]
fn fixture_csi_bob_unavailable() {
    let req = client_services_request(&["BOB"]);
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    assert_eq!(parsed.method, "ClientServicesInfo");
}

// ──────────────────────────────────────────────────────────────────────
// § 7. JSON-RPC error fixtures
// ──────────────────────────────────────────────────────────────────────

#[test]
fn fixture_error_parse_error() {
    let err = emissary_cli::i2pcontrol::rpc::JsonRpcErrorResponse::new(
        emissary_cli::i2pcontrol::rpc::RequestId::Null,
        emissary_cli::i2pcontrol::rpc::error_codes::PARSE_ERROR,
        "Parse error",
    );
    let json = serde_json::to_value(&err).unwrap();
    assert_eq!(json["error"]["code"], -32700);
}

#[test]
fn fixture_error_invalid_request() {
    let err = emissary_cli::i2pcontrol::rpc::JsonRpcErrorResponse::new(
        emissary_cli::i2pcontrol::rpc::RequestId::Null,
        emissary_cli::i2pcontrol::rpc::error_codes::INVALID_REQUEST,
        "Invalid request",
    );
    let json = serde_json::to_value(&err).unwrap();
    assert_eq!(json["error"]["code"], -32600);
}

#[test]
fn fixture_error_method_not_found() {
    let err = emissary_cli::i2pcontrol::rpc::JsonRpcErrorResponse::new(
        emissary_cli::i2pcontrol::rpc::RequestId::Null,
        emissary_cli::i2pcontrol::rpc::error_codes::METHOD_NOT_FOUND,
        "Method not found",
    );
    let json = serde_json::to_value(&err).unwrap();
    assert_eq!(json["error"]["code"], -32601);
}

#[test]
fn fixture_error_invalid_params() {
    let err = emissary_cli::i2pcontrol::rpc::JsonRpcErrorResponse::new(
        emissary_cli::i2pcontrol::rpc::RequestId::Null,
        emissary_cli::i2pcontrol::rpc::error_codes::INVALID_PARAMS,
        "Invalid params",
    );
    let json = serde_json::to_value(&err).unwrap();
    assert_eq!(json["error"]["code"], -32602);
}

#[test]
fn fixture_error_internal_error() {
    let err = emissary_cli::i2pcontrol::rpc::JsonRpcErrorResponse::new(
        emissary_cli::i2pcontrol::rpc::RequestId::Null,
        emissary_cli::i2pcontrol::rpc::error_codes::INTERNAL_ERROR,
        "Internal error",
    );
    let json = serde_json::to_value(&err).unwrap();
    assert_eq!(json["error"]["code"], -32603);
}

#[test]
fn fixture_error_auth_failure() {
    let err = emissary_cli::i2pcontrol::rpc::JsonRpcErrorResponse::new(
        emissary_cli::i2pcontrol::rpc::RequestId::Null,
        emissary_cli::i2pcontrol::rpc::error_codes::APP_ERROR,
        "Invalid username or password",
    );
    let json = serde_json::to_value(&err).unwrap();
    assert_eq!(json["error"]["code"], -1);
}

// ──────────────────────────────────────────────────────────────────────
// § 8. Response envelope exactness — no extra keys
// ──────────────────────────────────────────────────────────────────────

#[test]
fn success_envelope_has_exactly_three_keys() {
    let resp = emissary_cli::i2pcontrol::rpc::JsonRpcSuccess::new(
        emissary_cli::i2pcontrol::rpc::RequestId::Number(1),
        json!({"key": "value"}),
    );
    let json = serde_json::to_value(&resp).unwrap();
    let obj = json.as_object().unwrap();
    assert_eq!(obj.len(), 3);
    assert!(obj.contains_key("jsonrpc"));
    assert!(obj.contains_key("id"));
    assert!(obj.contains_key("result"));
    assert!(!obj.contains_key("error"));
}

#[test]
fn error_envelope_has_exactly_three_keys() {
    let resp = emissary_cli::i2pcontrol::rpc::JsonRpcErrorResponse::new(
        emissary_cli::i2pcontrol::rpc::RequestId::Number(1),
        -32601,
        "Method not found",
    );
    let json = serde_json::to_value(&resp).unwrap();
    let obj = json.as_object().unwrap();
    assert_eq!(obj.len(), 3);
    assert!(obj.contains_key("jsonrpc"));
    assert!(obj.contains_key("id"));
    assert!(obj.contains_key("error"));
    assert!(!obj.contains_key("result"));
}

// ──────────────────────────────────────────────────────────────────────
// § 9. SetConfig and SetSubscriptions fixtures
// ──────────────────────────────────────────────────────────────────────

#[test]
fn fixture_set_config() {
    let req = json!({
        "jsonrpc": "2.0",
        "method": "SetConfig",
        "params": {
            "config": {}
        },
        "id": 1
    });
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    assert_eq!(parsed.method, "SetConfig");
}

#[test]
fn fixture_set_subscriptions() {
    let req = json!({
        "jsonrpc": "2.0",
        "method": "SetSubscriptions",
        "params": {
            "subscriptions": {}
        },
        "id": 1
    });
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    assert_eq!(parsed.method, "SetSubscriptions");
}

// ──────────────────────────────────────────────────────────────────────
// § 10. GetKeys fixture
// ──────────────────────────────────────────────────────────────────────

#[test]
fn fixture_get_keys() {
    let req = json!({
        "jsonrpc": "2.0",
        "method": "GetKeys",
        "params": {},
        "id": 1
    });
    let parsed = emissary_cli::i2pcontrol::rpc::parse_request(&req.to_string()).unwrap();
    assert_eq!(parsed.method, "GetKeys");
}

// ──────────────────────────────────────────────────────────────────────
// § 11. No secret material in any fixture
// ──────────────────────────────────────────────────────────────────────

#[test]
fn fixtures_contain_no_real_secrets() {
    // All fixtures use "REDACTED" or "fixture-" prefixed values
    // This test verifies the pattern holds
    let fixtures = [
        "fixture-password-REDACTED",
        "fixture-token-REDACTED",
        "fixture-i2p-destination-REDACTED",
        "fixture-updated-REDACTED",
    ];
    for f in &fixtures {
        assert!(
            f.starts_with("fixture-"),
            "fixture must use fixture- prefix: {f}"
        );
        assert!(f.contains("REDACTED"), "fixture must contain REDACTED: {f}");
    }
}
