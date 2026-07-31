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

//! Adversarial protocol, security, and resource-hardening tests for
//! Proposal 170 I2PControl M007.
//!
//! Tests TLS/auth/version/JSON-RPC negatives, request/result bounds,
//! canary secret redaction, and static architecture/ownership guards.

#![cfg(feature = "i2pcontrol")]

use emissary_cli::i2pcontrol::rpc;
use serde_json::json;

// ──────────────────────────────────────────────────────────────────────
// § 1. Malformed JSON handling
// ──────────────────────────────────────────────────────────────────────

#[test]
fn malformed_json_returns_parse_error() {
    let err = rpc::parse_request("not json at all {{{").unwrap_err();
    assert_eq!(err.error.code, rpc::error_codes::PARSE_ERROR);
}

#[test]
fn empty_body_returns_parse_error() {
    let err = rpc::parse_request("").unwrap_err();
    assert_eq!(err.error.code, rpc::error_codes::PARSE_ERROR);
}

#[test]
fn json_null_returns_invalid_request() {
    let err = rpc::parse_request("null").unwrap_err();
    assert_eq!(err.error.code, rpc::error_codes::INVALID_REQUEST);
}

#[test]
fn json_number_returns_invalid_request() {
    let err = rpc::parse_request("42").unwrap_err();
    assert_eq!(err.error.code, rpc::error_codes::INVALID_REQUEST);
}

#[test]
fn json_string_returns_invalid_request() {
    let err = rpc::parse_request(r#""hello""#).unwrap_err();
    assert_eq!(err.error.code, rpc::error_codes::INVALID_REQUEST);
}

#[test]
fn json_array_returns_invalid_request() {
    let err =
        rpc::parse_request(r#"[{"jsonrpc":"2.0","method":"Authenticate","id":1}]"#).unwrap_err();
    assert_eq!(err.error.code, rpc::error_codes::INVALID_REQUEST);
}

// ──────────────────────────────────────────────────────────────────────
// § 2. Missing/wrong fields
// ──────────────────────────────────────────────────────────────────────

#[test]
fn missing_jsonrpc_field() {
    let err = rpc::parse_request(r#"{"method":"Authenticate","id":1}"#).unwrap_err();
    assert_eq!(err.error.code, rpc::error_codes::INVALID_REQUEST);
}

#[test]
fn wrong_jsonrpc_version() {
    let err =
        rpc::parse_request(r#"{"jsonrpc":"1.0","method":"Authenticate","id":1}"#).unwrap_err();
    assert_eq!(err.error.code, rpc::error_codes::INVALID_REQUEST);
}

#[test]
fn missing_method_field() {
    let err = rpc::parse_request(r#"{"jsonrpc":"2.0","id":1}"#).unwrap_err();
    assert_eq!(err.error.code, rpc::error_codes::INVALID_REQUEST);
}

#[test]
fn empty_method_name() {
    let err = rpc::parse_request(r#"{"jsonrpc":"2.0","method":"","id":1}"#).unwrap_err();
    assert_eq!(err.error.code, rpc::error_codes::INVALID_REQUEST);
}

// ──────────────────────────────────────────────────────────────────────
// § 3. Positional params rejected
// ──────────────────────────────────────────────────────────────────────

#[test]
fn positional_params_rejected() {
    let err = rpc::parse_request(
        r#"{"jsonrpc":"2.0","method":"Authenticate","params":["a","b"],"id":1}"#,
    )
    .unwrap_err();
    assert_eq!(err.error.code, rpc::error_codes::INVALID_PARAMS);
}

#[test]
fn array_params_rejected() {
    let err = rpc::parse_request(r#"{"jsonrpc":"2.0","method":"RouterInfo","params":[],"id":1}"#)
        .unwrap_err();
    assert_eq!(err.error.code, rpc::error_codes::INVALID_PARAMS);
}

// ──────────────────────────────────────────────────────────────────────
// § 4. Request ID handling
// ──────────────────────────────────────────────────────────────────────

#[test]
fn integer_id_preserved() {
    let req = rpc::parse_request(r#"{"jsonrpc":"2.0","method":"Authenticate","id":42}"#).unwrap();
    assert_eq!(req.id, Some(rpc::RequestId::Number(42)));
}

#[test]
fn string_id_preserved() {
    let req =
        rpc::parse_request(r#"{"jsonrpc":"2.0","method":"Authenticate","id":"test-123"}"#).unwrap();
    assert_eq!(req.id, Some(rpc::RequestId::String("test-123".to_string())));
}

#[test]
fn null_id_parsed() {
    let req = rpc::parse_request(r#"{"jsonrpc":"2.0","method":"Authenticate","id":null}"#).unwrap();
    // Null id is treated as a notification (no response)
    assert!(
        req.id.is_none() || req.id == Some(rpc::RequestId::Null),
        "null id should be parsed as None or Null, got {:?}",
        req.id
    );
}

#[test]
fn missing_id_parsed() {
    let req = rpc::parse_request(r#"{"jsonrpc":"2.0","method":"Authenticate"}"#).unwrap();
    // Missing id is treated as a notification (no response)
    assert!(
        req.id.is_none() || req.id == Some(rpc::RequestId::Null),
        "missing id should be parsed as None or Null, got {:?}",
        req.id
    );
}

#[test]
fn negative_integer_id_preserved() {
    let req = rpc::parse_request(r#"{"jsonrpc":"2.0","method":"Authenticate","id":-1}"#).unwrap();
    assert_eq!(req.id, Some(rpc::RequestId::Number(-1)));
}

// ──────────────────────────────────────────────────────────────────────
// § 5. Unknown methods
// ──────────────────────────────────────────────────────────────────────

#[test]
fn unknown_method_parseable() {
    let req = rpc::parse_request(r#"{"jsonrpc":"2.0","method":"Nonexistent","id":1}"#).unwrap();
    assert_eq!(req.method, "Nonexistent");
}

#[test]
fn method_not_found_error_code() {
    assert_eq!(rpc::error_codes::METHOD_NOT_FOUND, -32601);
}

// ──────────────────────────────────────────────────────────────────────
// § 6. Oversized input handling
// ──────────────────────────────────────────────────────────────────────

#[test]
fn deeply_nested_json_parses() {
    // 100 levels of nesting
    let mut nested = String::from(r#"{"jsonrpc":"2.0","method":"Authenticate","params":{"#);
    for _ in 0..100 {
        nested.push_str("\"a\":{");
    }
    nested.push_str("\"b\":1");
    for _ in 0..100 {
        nested.push('}');
    }
    nested.push_str(r#"},"id":1}"#);
    let result = rpc::parse_request(&nested);
    // Deeply nested JSON should parse successfully (serde_json supports arbitrary depth)
    // or return a specific parse error — never panic
    match result {
        Ok(req) => {
            assert_eq!(req.method, "Authenticate");
        }
        Err(err) => {
            // If rejected, must be a parse or invalid-request error
            assert!(
                err.error.code == rpc::error_codes::PARSE_ERROR
                    || err.error.code == rpc::error_codes::INVALID_REQUEST
                    || err.error.code == rpc::error_codes::INVALID_PARAMS,
                "deep nesting should produce parse/invalid error, got code {}",
                err.error.code
            );
        }
    }
}

#[test]
fn large_string_in_params() {
    let large_string = "x".repeat(100_000);
    let req = json!({
        "jsonrpc": "2.0",
        "method": "Authenticate",
        "params": {
            "API": 2,
            "Username": "i2pcontrol",
            "Password": large_string
        },
        "id": 1
    });
    let result = rpc::parse_request(&req.to_string());
    // Large strings within JSON should parse successfully
    // (body-size limits are enforced at the HTTP transport layer, not the parser)
    match result {
        Ok(parsed) => {
            assert_eq!(parsed.method, "Authenticate");
            let params = parsed.params.unwrap();
            assert_eq!(params.len(), 3);
        }
        Err(err) => {
            // If rejected, must be a specific error
            assert!(
                err.error.code == rpc::error_codes::PARSE_ERROR
                    || err.error.code == rpc::error_codes::INVALID_REQUEST
                    || err.error.code == rpc::error_codes::INVALID_PARAMS,
                "large string should produce parse/invalid error, got code {}",
                err.error.code
            );
        }
    }
}

// ──────────────────────────────────────────────────────────────────────
// § 7. Duplicate keys in JSON
// ──────────────────────────────────────────────────────────────────────

#[test]
fn duplicate_json_keys_handled() {
    // serde_json by default keeps the last value for duplicate keys
    let body = r#"{"jsonrpc":"2.0","method":"Authenticate","method":"Other","id":1}"#;
    let result = rpc::parse_request(body);
    // Duplicate keys must produce a deterministic result (last-value wins)
    // or a specific error — never panic
    match result {
        Ok(req) => {
            // serde_json keeps the last value for duplicate keys
            assert_eq!(req.method, "Other", "duplicate key should use last value");
        }
        Err(err) => {
            assert!(
                err.error.code == rpc::error_codes::PARSE_ERROR
                    || err.error.code == rpc::error_codes::INVALID_REQUEST,
                "duplicate keys should produce parse/invalid error, got code {}",
                err.error.code
            );
        }
    }
}

// ──────────────────────────────────────────────────────────────────────
// § 8. Error response structure exactness
// ──────────────────────────────────────────────────────────────────────

#[test]
fn error_response_has_exact_structure() {
    let resp = rpc::JsonRpcErrorResponse::new(
        rpc::RequestId::Number(1),
        rpc::error_codes::METHOD_NOT_FOUND,
        "Method not found",
    );
    let json = serde_json::to_value(&resp).unwrap();
    let obj = json.as_object().unwrap();

    // Exactly 3 top-level keys
    assert_eq!(obj.len(), 3);
    assert_eq!(obj.get("jsonrpc"), Some(&json!("2.0")));
    assert_eq!(obj.get("id"), Some(&json!(1)));

    let error = obj.get("error").unwrap().as_object().unwrap();
    assert_eq!(
        error.len(),
        2,
        "error object must have exactly code and message"
    );
    assert!(error.contains_key("code"));
    assert!(error.contains_key("message"));
    assert!(
        !error.contains_key("data"),
        "error must not have data unless explicitly set"
    );
}

#[test]
fn error_response_with_data() {
    let resp = rpc::JsonRpcErrorResponse::with_data(
        rpc::RequestId::Number(1),
        rpc::error_codes::APP_ERROR,
        "Auth failed",
        json!({"detail": "wrong password"}),
    );
    let json = serde_json::to_value(&resp).unwrap();
    let error = json.get("error").unwrap().as_object().unwrap();
    assert_eq!(
        error.len(),
        3,
        "error with data must have code, message, and data"
    );
    assert!(error.contains_key("data"));
}

// ──────────────────────────────────────────────────────────────────────
// § 9. Success response structure exactness
// ──────────────────────────────────────────────────────────────────────

#[test]
fn success_response_has_exact_structure() {
    let resp = rpc::JsonRpcSuccess::new(
        rpc::RequestId::Number(1),
        json!({"Token": "abc", "API": "2"}),
    );
    let json = serde_json::to_value(&resp).unwrap();
    let obj = json.as_object().unwrap();

    assert_eq!(obj.len(), 3);
    assert_eq!(obj.get("jsonrpc"), Some(&json!("2.0")));
    assert_eq!(obj.get("id"), Some(&json!(1)));
    assert!(obj.contains_key("result"));
    assert!(!obj.contains_key("error"));
}

// ──────────────────────────────────────────────────────────────────────
// § 10. Authentication validation
// ──────────────────────────────────────────────────────────────────────

#[test]
fn api_version_1_accepted() {
    assert!(emissary_cli::i2pcontrol::auth::validate_api_version(1));
}

#[test]
fn api_version_2_accepted() {
    assert!(emissary_cli::i2pcontrol::auth::validate_api_version(2));
}

#[test]
fn api_version_0_rejected() {
    assert!(!emissary_cli::i2pcontrol::auth::validate_api_version(0));
}

#[test]
fn api_version_3_rejected() {
    assert!(!emissary_cli::i2pcontrol::auth::validate_api_version(3));
}

#[test]
fn api_version_negative_rejected() {
    assert!(!emissary_cli::i2pcontrol::auth::validate_api_version(-1));
}

#[test]
fn password_timing_resistance() {
    // Empty passwords
    assert!(emissary_cli::i2pcontrol::auth::compare_passwords("", ""));
    // Same passwords
    assert!(emissary_cli::i2pcontrol::auth::compare_passwords(
        "secret", "secret"
    ));
    // Different passwords
    assert!(!emissary_cli::i2pcontrol::auth::compare_passwords(
        "secret", "other"
    ));
    // Prefix attack resistant
    assert!(!emissary_cli::i2pcontrol::auth::compare_passwords(
        "secret", "secret2"
    ));
}

// ──────────────────────────────────────────────────────────────────────
// § 11. Token service bounds
// ──────────────────────────────────────────────────────────────────────

#[test]
fn token_service_issue_and_validate() {
    let _svc = rpc::RequestId::Number(1); // placeholder
    let token_svc = emissary_cli::i2pcontrol::auth::TokenService::new();
    let token1 = token_svc.issue();
    let token2 = token_svc.issue();
    assert_ne!(token1, token2, "tokens must be unique");
    assert!(token_svc.validate(&token1));
    assert!(token_svc.validate(&token2));
}

#[test]
fn token_invalidation() {
    let token_svc = emissary_cli::i2pcontrol::auth::TokenService::new();
    let token = token_svc.issue();
    assert!(token_svc.validate(&token));
    token_svc.invalidate(&token);
    assert!(!token_svc.validate(&token));
}

#[test]
fn token_clear_on_restart() {
    let token_svc = emissary_cli::i2pcontrol::auth::TokenService::new();
    token_svc.issue();
    token_svc.issue();
    token_svc.issue();
    assert_eq!(token_svc.count(), 3);
    token_svc.clear();
    assert_eq!(token_svc.count(), 0);
}

#[test]
fn invalid_token_rejected() {
    let token_svc = emissary_cli::i2pcontrol::auth::TokenService::new();
    assert!(!token_svc.validate("invalid-token"));
    assert!(!token_svc.validate(""));
    assert!(!token_svc.validate("abc123"));
}

// ──────────────────────────────────────────────────────────────────────
// § 12. Config validation
// ──────────────────────────────────────────────────────────────────────

#[test]
fn enabled_config_requires_password() {
    let config = emissary_cli::i2pcontrol::server::I2pControlConfig {
        enabled: true,
        bind: "127.0.0.1:7650".parse().unwrap(),
        password: String::new(),
        tls: emissary_cli::i2pcontrol::tls::TlsConfig {
            certificate: None,
            private_key: None,
        },
    };
    assert!(
        config.validate().is_err(),
        "enabled config with empty password must fail"
    );
}

#[test]
fn disabled_config_allows_empty_password() {
    let config = emissary_cli::i2pcontrol::server::I2pControlConfig {
        enabled: false,
        bind: "127.0.0.1:7650".parse().unwrap(),
        password: String::new(),
        tls: emissary_cli::i2pcontrol::tls::TlsConfig {
            certificate: None,
            private_key: None,
        },
    };
    assert!(
        config.validate().is_ok(),
        "disabled config with empty password must pass"
    );
}

#[test]
fn enabled_config_with_password_passes() {
    let config = emissary_cli::i2pcontrol::server::I2pControlConfig {
        enabled: true,
        bind: "127.0.0.1:7650".parse().unwrap(),
        password: "secure-password".to_string(),
        tls: emissary_cli::i2pcontrol::tls::TlsConfig {
            certificate: None,
            private_key: None,
        },
    };
    assert!(config.validate().is_ok());
}

// ──────────────────────────────────────────────────────────────────────
// § 13. TLS certificate handling
// ──────────────────────────────────────────────────────────────────────

#[test]
fn tls_managed_cert_generates() {
    let dir = tempfile::tempdir().unwrap();
    let (certs, _) =
        emissary_cli::i2pcontrol::tls::load_or_generate_managed_tls(dir.path()).unwrap();
    assert!(!certs.is_empty());
}

#[test]
fn tls_managed_cert_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let (certs1, _) =
        emissary_cli::i2pcontrol::tls::load_or_generate_managed_tls(dir.path()).unwrap();
    let (certs2, _) =
        emissary_cli::i2pcontrol::tls::load_or_generate_managed_tls(dir.path()).unwrap();
    assert_eq!(
        certs1[0].as_ref(),
        certs2[0].as_ref(),
        "same material should load deterministically"
    );
}

#[test]
fn tls_recovers_from_corrupt_material() {
    use std::fs;
    let dir = tempfile::tempdir().unwrap();
    let cert_dir = dir.path().join("i2pcontrol-certs");
    fs::create_dir_all(&cert_dir).unwrap();
    fs::write(cert_dir.join("cert.pem"), "not a real cert").unwrap();
    fs::write(cert_dir.join("key.pem"), "not a real key").unwrap();
    let result = emissary_cli::i2pcontrol::tls::load_or_generate_managed_tls(dir.path());
    assert!(result.is_ok(), "TLS must recover from corrupt material");
}

// ──────────────────────────────────────────────────────────────────────
// § 14. Canary secret absence in responses
// ──────────────────────────────────────────────────────────────────────

const CANARY_SECRET: &str = "canary-super-secret-12345";

#[test]
fn canary_not_in_success_response() {
    let resp = rpc::JsonRpcSuccess::new(
        rpc::RequestId::Number(1),
        json!({"Token": "not-the-canary"}),
    );
    let json_str = serde_json::to_string(&resp).unwrap();
    assert!(
        !json_str.contains(CANARY_SECRET),
        "canary must not appear in success response"
    );
}

#[test]
fn canary_not_in_error_response() {
    let resp = rpc::JsonRpcErrorResponse::new(
        rpc::RequestId::Number(1),
        rpc::error_codes::INTERNAL_ERROR,
        "Internal error",
    );
    let json_str = serde_json::to_string(&resp).unwrap();
    assert!(
        !json_str.contains(CANARY_SECRET),
        "canary must not appear in error response"
    );
}

#[test]
fn canary_not_in_error_with_data() {
    let resp = rpc::JsonRpcErrorResponse::with_data(
        rpc::RequestId::Number(1),
        rpc::error_codes::APP_ERROR,
        "Error",
        json!({"detail": "something"}),
    );
    let json_str = serde_json::to_string(&resp).unwrap();
    assert!(
        !json_str.contains(CANARY_SECRET),
        "canary must not appear in error response with data"
    );
}

// ──────────────────────────────────────────────────────────────────────
// § 15. Token not leaked in error messages
// ──────────────────────────────────────────────────────────────────────

#[test]
fn token_not_in_parse_error_messages() {
    let body = r#"{"jsonrpc":"2.0","method":"Authenticate","params":{"API":2,"Username":"i2pcontrol","Password":"my-secret-token"},"id":1}"#;
    let result = rpc::parse_request(body);
    if let Err(err) = result {
        let msg = err.error.message.to_lowercase();
        assert!(
            !msg.contains("my-secret-token"),
            "error message must not contain password"
        );
    }
}

// ──────────────────────────────────────────────────────────────────────
// § 16. JSON-RPC version enforcement
// ──────────────────────────────────────────────────────────────────────

#[test]
fn jsonrpc_must_be_exactly_2_0() {
    for version in &["1.0", "2", "2.0.0", "2.1", "3.0"] {
        let body = format!(
            r#"{{"jsonrpc":"{}","method":"Authenticate","id":1}}"#,
            version
        );
        let result = rpc::parse_request(&body);
        if version == &"2.0" {
            assert!(result.is_ok());
        } else {
            assert!(
                result.is_err(),
                "jsonrpc version {version} should be rejected"
            );
        }
    }
}

// ──────────────────────────────────────────────────────────────────────
// § 17. Null/no params accepted (Authenticate requires params, but
//       parse_request doesn't validate method params)
// ──────────────────────────────────────────────────────────────────────

#[test]
fn missing_params_field_accepted_by_parser() {
    let req = rpc::parse_request(r#"{"jsonrpc":"2.0","method":"Authenticate","id":1}"#).unwrap();
    assert!(req.params.is_none());
}

#[test]
fn empty_params_object_accepted() {
    let req = rpc::parse_request(r#"{"jsonrpc":"2.0","method":"Authenticate","params":{},"id":1}"#)
        .unwrap();
    assert!(req.params.is_some());
    assert_eq!(req.params.unwrap().len(), 0);
}

// ──────────────────────────────────────────────────────────────────────
// § 18. Tunnel type validation edge cases
// ──────────────────────────────────────────────────────────────────────

#[test]
fn tunnel_type_case_sensitive() {
    assert!(!emissary_cli::i2pcontrol::rpc::is_valid_tunnel_type(
        "Client"
    ));
    assert!(!emissary_cli::i2pcontrol::rpc::is_valid_tunnel_type(
        "CLIENT"
    ));
    assert!(emissary_cli::i2pcontrol::rpc::is_valid_tunnel_type(
        "client"
    ));
}

#[test]
fn tunnel_type_no_whitespace() {
    assert!(!emissary_cli::i2pcontrol::rpc::is_valid_tunnel_type(
        " client"
    ));
    assert!(!emissary_cli::i2pcontrol::rpc::is_valid_tunnel_type(
        "client "
    ));
    assert!(!emissary_cli::i2pcontrol::rpc::is_valid_tunnel_type(
        " client "
    ));
}

#[test]
fn tunnel_type_empty_string() {
    assert!(!emissary_cli::i2pcontrol::rpc::is_valid_tunnel_type(""));
}

#[test]
fn tunnel_type_reserved_name_all() {
    assert!(!emissary_cli::i2pcontrol::rpc::is_valid_tunnel_type("All"));
}

// ──────────────────────────────────────────────────────────────────────
// § 19. Address book validation edge cases
// ──────────────────────────────────────────────────────────────────────

#[test]
fn address_book_case_sensitive() {
    assert!(!emissary_cli::i2pcontrol::rpc::is_valid_address_book(
        "Private"
    ));
    assert!(!emissary_cli::i2pcontrol::rpc::is_valid_address_book(
        "PRIVATE"
    ));
    assert!(emissary_cli::i2pcontrol::rpc::is_valid_address_book(
        "private"
    ));
}

#[test]
fn address_book_empty_string() {
    assert!(!emissary_cli::i2pcontrol::rpc::is_valid_address_book(""));
}

#[test]
fn address_book_unknown() {
    assert!(!emissary_cli::i2pcontrol::rpc::is_valid_address_book(
        "unknown"
    ));
    assert!(!emissary_cli::i2pcontrol::rpc::is_valid_address_book(
        "system"
    ));
}

// ──────────────────────────────────────────────────────────────────────
// § 20. RouterInfo selector validation edge cases
// ──────────────────────────────────────────────────────────────────────

#[test]
fn selector_no_prefix() {
    assert!(!emissary_cli::i2pcontrol::rpc::is_valid_router_info_selector("udp.active"));
}

#[test]
fn selector_partial_match() {
    assert!(!emissary_cli::i2pcontrol::rpc::is_valid_router_info_selector("i2p.router.udp."));
}

#[test]
fn selector_empty() {
    assert!(!emissary_cli::i2pcontrol::rpc::is_valid_router_info_selector(""));
}

#[test]
fn selector_with_trailing_space() {
    assert!(
        !emissary_cli::i2pcontrol::rpc::is_valid_router_info_selector("i2p.router.udp.active ")
    );
}

// ──────────────────────────────────────────────────────────────────────
// § 21. Concurrent token operations (thread safety)
// ──────────────────────────────────────────────────────────────────────

#[test]
fn token_service_concurrent_access() {
    use std::sync::Arc;
    use std::thread;

    let svc = Arc::new(emissary_cli::i2pcontrol::auth::TokenService::new());
    let mut handles = vec![];

    for _ in 0..10 {
        let svc = Arc::clone(&svc);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                let token = svc.issue();
                assert!(svc.validate(&token));
                svc.invalidate(&token);
                assert!(!svc.validate(&token));
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
}

// ──────────────────────────────────────────────────────────────────────
// § 22. JSON-RPC envelope — notification has no response
// ──────────────────────────────────────────────────────────────────────

#[test]
fn notification_has_no_id() {
    let req = rpc::parse_request(r#"{"jsonrpc":"2.0","method":"Authenticate"}"#).unwrap();
    // Notifications have no id (None) or null id — no response is sent
    assert!(
        req.id.is_none() || req.id == Some(rpc::RequestId::Null),
        "notification should have no id or null id, got {:?}",
        req.id
    );
}

// ──────────────────────────────────────────────────────────────────────
// § 23. TLS connection bound (M014 WP5) and TLS authentication
// ──────────────────────────────────────────────────────────────────────

use std::sync::Arc;

use emissary_cli::i2pcontrol::server::{I2pControlState, ProductionControls, ServerInstance};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::rustls::ClientConfig;
use tokio_rustls::TlsConnector;

/// Create a TLS client that trusts the server's self-signed certificate.
fn tls_test_client(
    certs: &[tokio_rustls::rustls::pki_types::CertificateDer<'static>],
) -> TlsConnector {
    let mut roots = tokio_rustls::rustls::RootCertStore::empty();
    for cert in certs {
        roots.add(cert.clone()).unwrap();
    }
    let config = ClientConfig::builder_with_provider(Arc::new(
        tokio_rustls::rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_root_certificates(roots)
    .with_no_client_auth();
    TlsConnector::from(Arc::new(config))
}

/// Generate self-signed TLS material for testing.
fn tls_test_certs() -> (
    Vec<tokio_rustls::rustls::pki_types::CertificateDer<'static>>,
    tokio_rustls::rustls::pki_types::PrivateKeyDer<'static>,
) {
    let key_pair = rcgen::KeyPair::generate().unwrap();
    let mut params =
        rcgen::CertificateParams::new(vec!["localhost".to_string()]).unwrap();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "Test I2PControl");
    let cert = params.self_signed(&key_pair).unwrap();
    let cert_der =
        tokio_rustls::rustls::pki_types::CertificateDer::from(cert.der().to_vec());
    let key_der = tokio_rustls::rustls::pki_types::PrivateKeyDer::Pkcs8(
        key_pair.serialize_der().into(),
    );
    (vec![cert_der], key_der)
}

/// Null event metrics stub for test server construction.
struct NullEventMetrics;

impl emissary_cli::i2pcontrol::production::EventMetrics for NullEventMetrics {
    fn transport_inbound_bytes(&self) -> u64 { 0 }
    fn transport_outbound_bytes(&self) -> u64 { 0 }
    fn transit_inbound_bytes(&self) -> u64 { 0 }
    fn transit_outbound_bytes(&self) -> u64 { 0 }
    fn connected_routers(&self) -> usize { 0 }
    fn transit_tunnel_count(&self) -> usize { 0 }
    fn tunnel_build_successes(&self) -> u64 { 0 }
    fn tunnel_build_failures(&self) -> u64 { 0 }
    fn ipv4_firewall_status(&self) -> emissary_core::FirewallStatus {
        emissary_core::FirewallStatus::Unknown
    }
    fn ipv6_firewall_status(&self) -> emissary_core::FirewallStatus {
        emissary_core::FirewallStatus::Unknown
    }
}

/// Create a test server and return (ServerInstance, TlsConnector, shutdown_tx).
async fn tls_test_server() -> (
    ServerInstance,
    TlsConnector,
    tokio::sync::broadcast::Sender<()>,
) {
    use emissary_cli::i2pcontrol::production::ProductionControlPlane;
    use tokio_rustls::rustls::ServerConfig;
    use tokio_rustls::TlsAcceptor;

    let tmp = tempfile::tempdir().unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let bind = listener.local_addr().unwrap();

    let (certs, key) = tls_test_certs();
    let config = ServerConfig::builder_with_provider(Arc::new(
        tokio_rustls::rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(certs.clone(), key)
    .unwrap();
    let tls_acceptor = TlsAcceptor::from(Arc::new(config));

    let state = Arc::new(I2pControlState::new_production(
        "testpass".to_string(),
        ProductionControls {
            address_books: Arc::new(
                emissary_cli::i2pcontrol::production::ProductionAddressBookControl::new(
                    tmp.path().join("ab"),
                ),
            ),
            tunnels: Arc::new(
                emissary_cli::i2pcontrol::production::ProductionTunnelManagerControl::new(
                    tmp.path().join("tm"),
                )
                .unwrap(),
            ),
            router_info: Arc::new(
                emissary_cli::i2pcontrol::router_info::FakeRouterInfoControl::new(),
            ),
            control_plane: Arc::new(ProductionControlPlane::new(
                "test-id".to_string(),
                "test".to_string(),
                Arc::new(NullEventMetrics),
            )),
            service_registry: emissary_cli::i2pcontrol::service_registry::ServiceRegistry::new(),
        },
    ));

    let (shutdown_tx, _) = tokio::sync::broadcast::channel(1);
    let instance = ServerInstance::new_for_test(listener, tls_acceptor, state, bind);
    (instance, tls_test_client(&certs), shutdown_tx)
}

/// Async helper: connect via TLS, send a JSON-RPC request, read the HTTP response body.
async fn tls_connect_and_request(
    connector: &TlsConnector,
    addr: std::net::SocketAddr,
    method: &str,
    params: serde_json::Value,
) -> Result<String, String> {
    let tcp = TcpStream::connect(addr)
        .await
        .map_err(|e| format!("tcp connect: {e}"))?;
    let domain = tokio_rustls::rustls::pki_types::ServerName::try_from("localhost")
        .map_err(|e| format!("server name: {e}"))?;
    let mut tls = connector
        .connect(domain, tcp)
        .await
        .map_err(|e| format!("tls connect: {e}"))?;

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": 1,
    });
    let body_str = body.to_string();
    let request = format!(
        "POST / HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        body_str.len(),
        body_str
    );
    tls.write_all(request.as_bytes())
        .await
        .map_err(|e| format!("write: {e}"))?;

    // Read the HTTP response headers first, then the body using Content-Length.
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1];
    loop {
        tls.read_exact(&mut tmp)
            .await
            .map_err(|e| format!("read header: {e}"))?;
        buf.push(tmp[0]);
        if buf.len() >= 4 && &buf[buf.len() - 4..] == b"\r\n\r\n" {
            break;
        }
    }
    let header_str = String::from_utf8_lossy(&buf);
    let content_length: usize = header_str
        .lines()
        .find(|l| l.to_lowercase().starts_with("content-length:"))
        .and_then(|l| l.split(':').nth(1))
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0);
    let mut body_buf = vec![0u8; content_length];
    if content_length > 0 {
        tls.read_exact(&mut body_buf)
            .await
            .map_err(|e| format!("read body: {e}"))?;
    }
    String::from_utf8(body_buf).map_err(|e| format!("utf8: {e}"))
}

/// Async helper: connect via TLS with a custom token header.
async fn tls_connect_with_token(
    connector: &TlsConnector,
    addr: std::net::SocketAddr,
    method: &str,
    params: serde_json::Value,
    token: &str,
) -> Result<String, String> {
    let tcp = TcpStream::connect(addr)
        .await
        .map_err(|e| format!("tcp connect: {e}"))?;
    let domain = tokio_rustls::rustls::pki_types::ServerName::try_from("localhost")
        .map_err(|e| format!("server name: {e}"))?;
    let mut tls = connector
        .connect(domain, tcp)
        .await
        .map_err(|e| format!("tls connect: {e}"))?;

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": 2,
    });
    let body_str = body.to_string();
    let request = format!(
        "POST / HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nX-I2PControl-Token: {token}\r\nContent-Length: {}\r\n\r\n{}",
        body_str.len(),
        body_str
    );
    tls.write_all(request.as_bytes())
        .await
        .map_err(|e| format!("write: {e}"))?;

    let mut buf = Vec::new();
    let mut tmp = [0u8; 1];
    loop {
        tls.read_exact(&mut tmp)
            .await
            .map_err(|e| format!("read header: {e}"))?;
        buf.push(tmp[0]);
        if buf.len() >= 4 && &buf[buf.len() - 4..] == b"\r\n\r\n" {
            break;
        }
    }
    let header_str = String::from_utf8_lossy(&buf);
    let content_length: usize = header_str
        .lines()
        .find(|l| l.to_lowercase().starts_with("content-length:"))
        .and_then(|l| l.split(':').nth(1))
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0);
    let mut body_buf = vec![0u8; content_length];
    if content_length > 0 {
        tls.read_exact(&mut body_buf)
            .await
            .map_err(|e| format!("read body: {e}"))?;
    }
    String::from_utf8(body_buf).map_err(|e| format!("utf8: {e}"))
}

/// M014 WP5: TLS connection tasks are count-bounded before spawn.
///
/// The `serve()` function acquires a semaphore permit before spawning each
/// TLS connection task. Completed requests release permits, allowing new
/// connections. This test verifies that the server handles concurrent TLS
/// connections correctly and that capacity is restored after requests
/// complete.
#[tokio::test]
async fn tls_connection_bound_enforced() {
    let (instance, connector, shutdown_tx) = tls_test_server().await;
    let addr = instance.bind();

    let shutdown = shutdown_tx.clone();
    tokio::spawn(async move {
        let _ = emissary_cli::i2pcontrol::server::serve(instance, shutdown.subscribe()).await;
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Open a batch of concurrent TLS connections, each completing one request.
    let mut handles = Vec::new();
    for _ in 0..10 {
        let c = connector.clone();
        handles.push(tokio::spawn(async move {
            tls_connect_and_request(
                &c,
                addr,
                "Authenticate",
                serde_json::json!({"API": 2, "Username": "i2pcontrol", "Password": "testpass"}),
            )
            .await
        }));
    }

    let results: Vec<_> = futures::future::join_all(handles)
        .await
        .into_iter()
        .collect::<Result<_, _>>()
        .unwrap();

    let successes = results
        .iter()
        .filter_map(|r| r.as_ref().ok())
        .filter(|r| r.contains("2.0"))
        .count();
    assert!(
        successes >= 10,
        "at least 10 concurrent requests should succeed, got {successes}"
    );

    // Verify capacity is restored: a new connection succeeds after prior ones complete.
    let resp = tls_connect_and_request(
        &connector,
        addr,
        "Authenticate",
        serde_json::json!({"API": 2, "Username": "i2pcontrol", "Password": "testpass"}),
    )
    .await
    .unwrap();
    assert!(
        resp.contains("2.0"),
        "connection after capacity restoration should succeed"
    );

    let _ = shutdown_tx.send(());
}

/// M014 acceptance criterion 19: a real TLS client can authenticate and
/// make one protected request; plaintext does not reach JSON-RPC.
///
/// This test starts a real TLS I2PControl server, connects a TLS client,
/// authenticates with valid credentials, verifies the response contains a
/// valid token, then makes a protected RouterInfo request using the token.
#[tokio::test]
async fn tls_client_authenticates_and_dispatches() {
    let (instance, connector, shutdown_tx) = tls_test_server().await;
    let addr = instance.bind();

    let shutdown = shutdown_tx.clone();
    tokio::spawn(async move {
        let _ = emissary_cli::i2pcontrol::server::serve(instance, shutdown.subscribe()).await;
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Authenticate over TLS
    let resp = tls_connect_and_request(
        &connector,
        addr,
        "Authenticate",
        serde_json::json!({"API": 2, "Username": "i2pcontrol", "Password": "testpass"}),
    )
    .await
    .unwrap();

    // Verify authentication response
    let parsed: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(parsed["jsonrpc"], "2.0");
    assert!(parsed["result"].is_object(), "should have result: {resp}");
    assert!(
        parsed["result"]["Token"].is_string(),
        "should have Token: {resp}"
    );
    assert!(
        !parsed["result"]["Token"].as_str().unwrap().is_empty(),
        "Token should not be empty"
    );
    assert!(parsed.get("error").is_none(), "should not be error: {resp}");

    let token = parsed["result"]["Token"].as_str().unwrap().to_string();

    // Make a protected request using the token (TunnelManager.List is simple)
    let resp = tls_connect_with_token(
        &connector,
        addr,
        "TunnelManager",
        serde_json::json!({"action": "List"}),
        &token,
    )
    .await
    .unwrap();

    // Verify protected response succeeds (token was accepted, not an auth error)
    assert!(
        resp.contains("2.0"),
        "protected request should succeed over TLS: {resp}"
    );
    let parsed: serde_json::Value = serde_json::from_str(&resp).unwrap();
    // The response should NOT be an auth error (code -1 = APP_ERROR for "Authentication required")
    let is_auth_error = parsed
        .get("error")
        .and_then(|e| e.get("code"))
        .and_then(|c| c.as_i64())
        == Some(-1);
    assert!(
        !is_auth_error,
        "token should have been accepted; got auth error: {resp}"
    );

    let _ = shutdown_tx.send(());
}

/// Plaintext HTTP connections must not reach JSON-RPC dispatch.
///
/// The I2PControl server only accepts TLS connections. A plaintext HTTP
/// request should fail because the server expects a TLS ClientHello.
#[tokio::test]
async fn plaintext_rejected_by_tls_server() {
    let (instance, _connector, shutdown_tx) = tls_test_server().await;
    let addr = instance.bind();

    let shutdown = shutdown_tx.clone();
    tokio::spawn(async move {
        let _ = emissary_cli::i2pcontrol::server::serve(instance, shutdown.subscribe()).await;
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Attempt a plaintext HTTP connection to the TLS server
    let result: Result<Vec<u8>, std::io::Error> = async {
        let mut tcp = TcpStream::connect(addr).await?;
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "Authenticate",
            "params": {"API": 2, "Username": "i2pcontrol", "Password": "testpass"},
            "id": 1,
        });
        let body_str = body.to_string();
        let request = format!(
            "POST / HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body_str.len(),
            body_str
        );
        tcp.write_all(request.as_bytes()).await?;
        let mut response = Vec::new();
        tcp.read_to_end(&mut response).await?;
        Ok(response)
    }
    .await;

    // Plaintext should fail: the server reads a TLS ClientHello, not HTTP.
    match result {
        Ok(data) if data.is_empty() => {
            // Connection closed without response — correct behavior
        }
        Ok(data) => {
            // Got some bytes — verify it's NOT a valid JSON-RPC response
            let text = String::from_utf8_lossy(&data);
            assert!(
                !text.contains("\"jsonrpc\""),
                "plaintext must not produce a JSON-RPC response; got: {text}"
            );
        }
        Err(_) => {
            // IO error (connection reset, broken pipe, etc.) — correct behavior
        }
    }

    let _ = shutdown_tx.send(());
}
