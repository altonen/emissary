mod fixtures;

use emissary_cli::i2pcontrol::control_plane::ControlPlane;
use fixtures::*;
use serde_json::json;

/// Parse a valid request and verify structure.
#[test]
fn parse_valid_authenticate_request() {
    let body = valid_authenticate_request(json!(1));
    let req = emissary_cli::i2pcontrol::rpc::parse_request(&body.to_string()).unwrap();
    assert_eq!(req.jsonrpc, "2.0");
    assert_eq!(req.method, "Authenticate");
    assert!(req.params.is_some());
}

/// Missing jsonrpc returns parse error.
#[test]
fn parse_missing_jsonrpc_returns_error() {
    let body = missing_jsonrpc();
    let err = emissary_cli::i2pcontrol::rpc::parse_request(&body.to_string()).unwrap_err();
    assert_eq!(
        err.error.code,
        emissary_cli::i2pcontrol::rpc::error_codes::INVALID_REQUEST
    );
}

/// Wrong jsonrpc version returns error.
#[test]
fn parse_wrong_version_returns_error() {
    let body = wrong_jsonrpc_version();
    let err = emissary_cli::i2pcontrol::rpc::parse_request(&body.to_string()).unwrap_err();
    assert_eq!(
        err.error.code,
        emissary_cli::i2pcontrol::rpc::error_codes::INVALID_REQUEST
    );
}

/// Missing method returns error.
#[test]
fn parse_missing_method_returns_error() {
    let body = missing_method();
    let err = emissary_cli::i2pcontrol::rpc::parse_request(&body.to_string()).unwrap_err();
    assert_eq!(
        err.error.code,
        emissary_cli::i2pcontrol::rpc::error_codes::INVALID_REQUEST
    );
}

/// Empty method returns error.
#[test]
fn parse_empty_method_returns_error() {
    let body = empty_method();
    let err = emissary_cli::i2pcontrol::rpc::parse_request(&body.to_string()).unwrap_err();
    assert_eq!(
        err.error.code,
        emissary_cli::i2pcontrol::rpc::error_codes::INVALID_REQUEST
    );
}

/// Positional params return error.
#[test]
fn parse_positional_params_returns_error() {
    let body = positional_params();
    let err = emissary_cli::i2pcontrol::rpc::parse_request(&body.to_string()).unwrap_err();
    assert_eq!(
        err.error.code,
        emissary_cli::i2pcontrol::rpc::error_codes::INVALID_PARAMS
    );
}

/// Unknown method is parseable (dispatch handles unknown).
#[test]
fn parse_unknown_method_succeeds() {
    let body = unknown_method();
    let req = emissary_cli::i2pcontrol::rpc::parse_request(&body.to_string()).unwrap();
    assert_eq!(req.method, "UnknownMethod");
}

/// Notification (null id) is parseable.
#[test]
fn parse_notification_succeeds() {
    let body = notification();
    let req = emissary_cli::i2pcontrol::rpc::parse_request(&body.to_string()).unwrap();
    assert!(req.id.is_none() || req.id == Some(emissary_cli::i2pcontrol::rpc::RequestId::Null));
}

/// String ID is preserved.
#[test]
fn parse_string_id_preserved() {
    let body = string_id();
    let req = emissary_cli::i2pcontrol::rpc::parse_request(&body.to_string()).unwrap();
    assert_eq!(
        req.id,
        Some(emissary_cli::i2pcontrol::rpc::RequestId::String(
            "abc-123".to_string()
        ))
    );
}

/// Non-JSON returns parse error.
#[test]
fn parse_non_json_returns_error() {
    let err = emissary_cli::i2pcontrol::rpc::parse_request(not_json()).unwrap_err();
    assert_eq!(
        err.error.code,
        emissary_cli::i2pcontrol::rpc::error_codes::PARSE_ERROR
    );
}

/// JSON array returns invalid request error.
#[test]
fn parse_json_array_returns_error() {
    let body = json_array();
    let err = emissary_cli::i2pcontrol::rpc::parse_request(&body.to_string()).unwrap_err();
    assert_eq!(
        err.error.code,
        emissary_cli::i2pcontrol::rpc::error_codes::INVALID_REQUEST
    );
}

/// Token service issue/validate cycle.
#[test]
fn token_service_basic() {
    let svc = emissary_cli::i2pcontrol::auth::TokenService::new();
    let token = svc.issue();
    assert!(svc.validate(&token));
    assert!(!svc.validate("invalid"));
    svc.invalidate(&token);
    assert!(!svc.validate(&token));
}

/// Token service clears on restart.
#[test]
fn token_service_clear() {
    let svc = emissary_cli::i2pcontrol::auth::TokenService::new();
    svc.issue();
    svc.issue();
    assert_eq!(svc.count(), 2);
    svc.clear();
    assert_eq!(svc.count(), 0);
}

/// API version validation.
#[test]
fn api_version_valid() {
    assert!(emissary_cli::i2pcontrol::auth::validate_api_version(1));
    assert!(emissary_cli::i2pcontrol::auth::validate_api_version(2));
    assert!(!emissary_cli::i2pcontrol::auth::validate_api_version(0));
    assert!(!emissary_cli::i2pcontrol::auth::validate_api_version(3));
}

/// Password comparison is timing-resistant.
#[test]
fn password_comparison() {
    assert!(emissary_cli::i2pcontrol::auth::compare_passwords(
        "secret", "secret"
    ));
    assert!(!emissary_cli::i2pcontrol::auth::compare_passwords(
        "secret", "other"
    ));
    assert!(emissary_cli::i2pcontrol::auth::compare_passwords("", ""));
}

/// Config validation rejects empty password when enabled.
#[test]
fn config_validation_rejects_empty_password() {
    let config = emissary_cli::i2pcontrol::server::I2pControlConfig {
        enabled: true,
        bind: "127.0.0.1:7650".parse().unwrap(),
        password: String::new(),
        tls: emissary_cli::i2pcontrol::tls::TlsConfig {
            certificate: None,
            private_key: None,
        },
    };
    assert!(config.validate().is_err());
}

/// Config validation accepts disabled with empty password.
#[test]
fn config_validation_accepts_disabled() {
    let config = emissary_cli::i2pcontrol::server::I2pControlConfig {
        enabled: false,
        bind: "127.0.0.1:7650".parse().unwrap(),
        password: String::new(),
        tls: emissary_cli::i2pcontrol::tls::TlsConfig {
            certificate: None,
            private_key: None,
        },
    };
    assert!(config.validate().is_ok());
}

/// Tunnel types inventory is complete.
#[test]
fn tunnel_types_inventory_complete() {
    let types = all_tunnel_types();
    assert_eq!(types.len(), 12);
    for tt in types {
        assert!(
            emissary_cli::i2pcontrol::rpc::is_valid_tunnel_type(tt),
            "Missing tunnel type: {tt}"
        );
    }
}

/// Address books inventory is complete.
#[test]
fn address_books_inventory_complete() {
    let books = all_address_books();
    assert_eq!(books.len(), 4);
    for ab in books {
        assert!(
            emissary_cli::i2pcontrol::rpc::is_valid_address_book(ab),
            "Missing address book: {ab}"
        );
    }
}

/// Tunnel actions inventory is complete.
#[test]
fn tunnel_actions_inventory_complete() {
    let actions = all_tunnel_actions();
    assert_eq!(actions.len(), 8);
}

/// Method constants are correct.
#[test]
fn method_constants_correct() {
    assert_eq!(
        emissary_cli::i2pcontrol::rpc::methods::AUTHENTICATE,
        "Authenticate"
    );
    assert_eq!(
        emissary_cli::i2pcontrol::rpc::methods::ROUTER_INFO,
        "RouterInfo"
    );
    assert_eq!(
        emissary_cli::i2pcontrol::rpc::methods::ADDRESS_BOOK,
        "AddressBook"
    );
    assert_eq!(
        emissary_cli::i2pcontrol::rpc::methods::TUNNEL_MANAGER,
        "TunnelManager"
    );
    assert_eq!(
        emissary_cli::i2pcontrol::rpc::methods::CLIENT_SERVICES_INFO,
        "ClientServicesInfo"
    );
    assert_eq!(emissary_cli::i2pcontrol::rpc::methods::GET_KEYS, "GetKeys");
    assert_eq!(
        emissary_cli::i2pcontrol::rpc::methods::SET_CONFIG,
        "SetConfig"
    );
    assert_eq!(
        emissary_cli::i2pcontrol::rpc::methods::SET_SUBSCRIPTIONS,
        "SetSubscriptions"
    );
}

/// Fake control plane returns stub values.
#[test]
fn fake_control_plane() {
    let cp = emissary_cli::i2pcontrol::control_plane::FakeControlPlane::new();
    assert_eq!(cp.router_version(), "Emissary 0.4.0");
    assert!(cp.router_identity().unwrap().is_empty());
    assert_eq!(cp.router_uptime_ms(), 0);
}

/// TLS managed certificate generates and loads.
#[test]
fn tls_managed_certificate_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();
    let (certs1, _) = emissary_cli::i2pcontrol::tls::load_or_generate_managed_tls(base).unwrap();
    assert!(!certs1.is_empty());
    let (certs2, _) = emissary_cli::i2pcontrol::tls::load_or_generate_managed_tls(base).unwrap();
    assert_eq!(certs1[0].as_ref(), certs2[0].as_ref());
}

/// TLS recovers from invalid material.
#[test]
fn tls_recovers_from_invalid_material() {
    use std::fs;
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();
    let cert_dir = base.join("i2pcontrol-certs");
    fs::create_dir_all(&cert_dir).unwrap();
    fs::write(cert_dir.join("cert.pem"), "not a cert").unwrap();
    fs::write(cert_dir.join("key.pem"), "not a key").unwrap();
    let result = emissary_cli::i2pcontrol::tls::load_or_generate_managed_tls(base);
    assert!(result.is_ok());
}

/// JSON-RPC success response serializes correctly.
#[test]
fn jsonrpc_success_response_serializes() {
    let resp = emissary_cli::i2pcontrol::rpc::JsonRpcSuccess::new(
        emissary_cli::i2pcontrol::rpc::RequestId::Number(1),
        json!({"Token": "abc", "API": "2"}),
    );
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"jsonrpc\":\"2.0\""));
    assert!(json.contains("\"result\""));
    assert!(!json.contains("\"error\""));
}

/// JSON-RPC error response serializes correctly.
#[test]
fn jsonrpc_error_response_serializes() {
    let resp = emissary_cli::i2pcontrol::rpc::JsonRpcErrorResponse::new(
        emissary_cli::i2pcontrol::rpc::RequestId::Number(1),
        emissary_cli::i2pcontrol::rpc::error_codes::METHOD_NOT_FOUND,
        "Method not found",
    );
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"error\""));
    assert!(json.contains("-32601"));
}

/// i2pcontrol module is not required for default builds.
#[test]
fn no_i2pcontrol_in_core() {
    // Verify emissary-core does not gain HTTP/TLS/JSON server deps
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let core_manifest_path = manifest.parent().unwrap().join("emissary-core/Cargo.toml");
    let core_manifest = std::fs::read_to_string(&core_manifest_path).unwrap();
    assert!(!core_manifest.contains("axum"));
    assert!(!core_manifest.contains("tokio-rustls"));
    assert!(!core_manifest.contains("rustls"));
}
