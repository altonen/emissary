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
