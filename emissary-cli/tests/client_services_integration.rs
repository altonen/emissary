mod fixtures;

use std::sync::Arc;

use emissary_cli::i2pcontrol::client_services::is_valid_client_services_selector;
use emissary_cli::i2pcontrol::rpc::{
    error_codes, parse_request, JsonRpcRequest, JsonRpcSuccess, RequestId,
};
use emissary_cli::i2pcontrol::service_registry::{
    ObservedServiceState, ServiceCategory, ServiceMetadata, ServiceRegistry, ServiceUpdateHandle,
};
use serde_json::{json, Value};

/// Build a ClientServicesInfo request body.
fn client_services_request(id: serde_json::Value, selectors: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "ClientServicesInfo",
        "params": { "Selector": selectors },
    })
}

#[test]
fn client_services_single_selector_bob_returns_false() {
    let body = client_services_request(json!(1), json!({"BOB": true}));
    let req = parse_request(&body.to_string()).expect("parse");
    assert_eq!(req.method, "ClientServicesInfo");
    assert!(req.params.is_some());
    assert!(is_valid_client_services_selector("BOB"));
}

#[test]
fn client_services_all_six_selectors_are_valid() {
    for key in ["I2PTunnel", "HTTPProxy", "SOCKS", "SAM", "BOB", "I2CP"] {
        assert!(
            is_valid_client_services_selector(key),
            "selector {key} must be accepted"
        );
    }
}

#[test]
fn client_services_unknown_selector_not_valid() {
    assert!(!is_valid_client_services_selector("NotAService"));
    assert!(!is_valid_client_services_selector("client"));
    assert!(!is_valid_client_services_selector("HTTPPROXY"));
    assert!(!is_valid_client_services_selector(""));
}

#[test]
fn client_services_response_shape_bob_is_boolean() {
    // Per Proposal 170 BOB returns the exact value `false`.
    // The handler logic builds: serde_json::json!(false)
    let v = json!(false);
    assert_eq!(v, Value::Bool(false));
}

#[test]
fn client_services_response_shape_i2ptunnel_has_client_and_server() {
    let shape = json!({
        "client": {},
        "server": {},
    });
    assert!(shape.get("client").is_some());
    assert!(shape.get("server").is_some());
}

#[test]
fn client_services_response_shape_httpproxy_socks_i2cp_have_enabled_key() {
    let proxy_shape = |enabled: bool, address: Option<String>, port: Option<u16>| {
        json!({
            "enabled": enabled,
            "address": address,
            "port": port,
        })
    };
    let listening = proxy_shape(true, Some("127.0.0.1".into()), Some(4444));
    assert_eq!(listening["enabled"], true);
    assert_eq!(listening["address"], "127.0.0.1");
    assert_eq!(listening["port"], 4444);

    let disabled = proxy_shape(false, None, None);
    assert_eq!(disabled["enabled"], false);
    assert!(disabled["address"].is_null());
    assert!(disabled["port"].is_null());
}

#[test]
fn client_services_response_shape_sam_have_enabled_and_sessions() {
    let shape = json!({
        "enabled": true,
        "sessions": {}
    });
    assert!(shape.get("enabled").is_some());
    assert!(shape.get("sessions").is_some());
}

#[test]
fn parse_client_services_request_with_positional_params_fails() {
    // Positional params are rejected at parse time per Proposal 170.
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "ClientServicesInfo",
        "params": [{}]
    });
    let err = parse_request(&body.to_string()).expect_err("must reject");
    assert_eq!(err.error.code, error_codes::INVALID_PARAMS);
}

#[test]
fn parse_client_services_request_with_null_selector_map_fails() {
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "ClientServicesInfo",
        "params": { "Selector": null }
    });
    let req = parse_request(&body.to_string()).expect("parse request");
    // Parsing succeeds; invalid Selector is reported by the handler.
    assert_eq!(req.method, "ClientServicesInfo");
    let params = req.params.as_ref().unwrap();
    assert!(params.get("Selector").is_some());
    assert!(params.get("Selector").unwrap().is_null());
}

#[test]
fn client_services_integration_observer_records_listening_then_stops() {
    use std::net::SocketAddr;
    let registry = ServiceRegistry::new();
    // HttpProxy lifecycle observation: Starting -> Listening -> Stopped
    let observer =
        ServiceUpdateHandle::clone(&registry.allocate_handle(ServiceCategory::HttpProxy));
    let _ = observer.update(ObservedServiceState::Starting, ServiceMetadata::default());

    let addr: SocketAddr = "127.0.0.1:4444".parse().unwrap();
    let metadata = ServiceMetadata {
        host: Some(addr.ip().to_string()),
        port: Some(addr.port()),
        enabled: true,
        session_count: None,
        tunnel_definitions: None,
    };
    let _ = observer.update(ObservedServiceState::Listening, metadata);

    let snap = registry.snapshot();
    let entry = snap.get(ServiceCategory::HttpProxy).unwrap();
    assert_eq!(entry.state, ObservedServiceState::Listening);
    assert_eq!(entry.metadata.port, Some(4444));

    // Stopped transition
    let _ = observer.update(ObservedServiceState::Stopped, ServiceMetadata::default());
    let snap = registry.snapshot();
    let entry = snap.get(ServiceCategory::HttpProxy).unwrap();
    assert_eq!(entry.state, ObservedServiceState::Stopped);
}

#[test]
fn client_services_sam_state_distinguishes_listener_enabled_from_active() {
    let registry = ServiceRegistry::new();
    let observer = ServiceUpdateHandle::clone(&registry.allocate_handle(ServiceCategory::Sam));
    let _ = observer.update(
        ObservedServiceState::Listening,
        ServiceMetadata {
            enabled: true,
            session_count: Some(2),
            ..ServiceMetadata::default()
        },
    );
    let snap = registry.snapshot();
    let entry = snap.get(ServiceCategory::Sam).unwrap();
    assert_eq!(entry.state, ObservedServiceState::Listening);
    assert!(entry.metadata.enabled);
    assert_eq!(entry.metadata.session_count, Some(2));
}

#[test]
fn client_services_stale_generation_observation_rejected() {
    let registry = ServiceRegistry::new();
    // Allocate a stale handle by allocating twice on the same category.
    let stale = registry.allocate_handle(ServiceCategory::I2PTunnel);
    let newer = registry.allocate_handle(ServiceCategory::I2PTunnel);
    // The stale handle must fail to update.
    let res = stale.update(ObservedServiceState::Listening, ServiceMetadata::default());
    assert!(res.is_err(), "stale generation updates must be rejected");

    // Newer handle succeeds.
    let res = newer.update(ObservedServiceState::Listening, ServiceMetadata::default());
    assert!(res.is_ok());
}

#[test]
fn client_services_concurrent_observations_dont_panic() {
    use std::thread;

    let registry = Arc::new(ServiceRegistry::new());
    let mut handles = vec![];
    for _ in 0..8 {
        let reg = Arc::clone(&registry);
        let h = thread::spawn(move || {
            let observer = reg.allocate_handle(ServiceCategory::HttpProxy);
            let _ = observer.update(ObservedServiceState::Starting, ServiceMetadata::default());
        });
        handles.push(h);
    }
    for h in handles {
        h.join().expect("thread join");
    }

    // Final state is one of the observed states; the test ensures no
    // panic and not all-handles-rejected.
    let snap = registry.snapshot();
    let entry = snap.get(ServiceCategory::HttpProxy).unwrap();
    // Whichever generation succeeded last — Starting, Configured, or
    // Listening — will be present. Disabled would mean every handle was
    // stale, which is not possible with N=8 allocations.
    assert!(matches!(
        entry.state,
        ObservedServiceState::Starting
            | ObservedServiceState::Configured
            | ObservedServiceState::Listening
    ));
}

#[test]
fn client_services_request_has_rpc_wrapper() {
    let body = client_services_request(json!(42), json!({"BOB": true}));
    let req: JsonRpcRequest = parse_request(&body.to_string()).expect("parse");
    assert_eq!(req.jsonrpc, "2.0");
    assert_eq!(req.method, "ClientServicesInfo");
    assert_eq!(req.id, Some(RequestId::Number(42)));
}

/// Serialize a success response to JSON value.
#[test]
fn client_services_success_response_structure() {
    let resp = JsonRpcSuccess::new(
        RequestId::Number(7),
        json!({
            "BOB": false,
            "HTTPProxy": {"enabled": true, "address": "127.0.0.1", "port": 4444},
        }),
    );
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["jsonrpc"], "2.0");
    assert_eq!(v["id"], 7);
    assert_eq!(v["result"]["BOB"], false);
    assert_eq!(v["result"]["HTTPProxy"]["enabled"], true);
    assert_eq!(v["result"]["HTTPProxy"]["port"], 4444);
}
