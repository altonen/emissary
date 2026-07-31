//! M011 live-state tests for ClientServicesInfo.
//!
//! These tests exercise the production handler path with live tunnel
//! manager queries, listener lifecycle transitions, and cross-method
//! consistency. They prove temporal behavior that unit tests on the
//! registry alone cannot demonstrate.

#![cfg(feature = "i2pcontrol")]

use emissary_cli::i2pcontrol::{
    control_plane::{FakeTunnelManagerControl, TunnelManagerControl},
    domain::tunnel::{
        StartIntent, TunnelDefinition, TunnelName, TunnelOptions, TunnelOwnership,
        TunnelRuntimeState, TunnelType,
    },
    rpc::{parse_request, JsonRpcRequest},
    server::I2pControlState,
    service_registry::{ObservedServiceState, ServiceCategory, ServiceMetadata, ServiceRegistry},
};
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_state(reg: ServiceRegistry) -> I2pControlState {
    let mut state = I2pControlState::new_for_test("test".to_string());
    state.set_service_registry(reg);
    state
}

fn cs_request(id: Value, selectors: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "ClientServicesInfo",
        "params": { "Selector": selectors },
    })
}

fn make_tunnel_def(
    name: &str,
    tunnel_type: TunnelType,
    target: Option<String>,
    host: Option<String>,
    port: Option<u16>,
) -> TunnelDefinition {
    TunnelDefinition {
        name: TunnelName::new(name).unwrap(),
        tunnel_type,
        ownership: TunnelOwnership::ControlPlane,
        runtime_state: TunnelRuntimeState::Stopped,
        start_intent: StartIntent::DoNotStart,
        options: TunnelOptions {
            target_destination: target,
            hosting_destination: host,
            listen_port: port,
            ..Default::default()
        },
        raw_config: Default::default(),
    }
}

async fn assemble_response_live(state: &I2pControlState, req: &JsonRpcRequest) -> Value {
    let snapshot = state.service_snapshot();
    let tm = state.tunnel_manager();
    let mut requested_keys: Vec<&str> = Vec::new();
    let params = req.params.as_ref().unwrap();
    let selector_map = params.get("Selector").unwrap().as_object().unwrap();
    for (key, value) in selector_map {
        if value.as_bool() == Some(true) {
            requested_keys.push(key.as_str());
        }
    }
    match emissary_cli::i2pcontrol::client_services::assemble_response_with_observation(
        &snapshot,
        &requested_keys,
        tm,
        state.sam_session_observation(),
    )
    .await
    {
        Ok(map) => {
            let response = emissary_cli::i2pcontrol::rpc::JsonRpcSuccess::new(
                req.id.clone().unwrap_or(emissary_cli::i2pcontrol::rpc::RequestId::Null),
                serde_json::Value::Object(map),
            );
            serde_json::to_value(&response).unwrap()
        }
        Err(e) => {
            let response = emissary_cli::i2pcontrol::rpc::JsonRpcErrorResponse::new(
                req.id.clone().unwrap_or(emissary_cli::i2pcontrol::rpc::RequestId::Null),
                emissary_cli::i2pcontrol::rpc::error_codes::INTERNAL_ERROR,
                e,
            );
            serde_json::to_value(&response).unwrap()
        }
    }
}

// ---------------------------------------------------------------------------
// I2PTunnel live query tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn i2ptunnel_empty_inventory_live() {
    let reg = ServiceRegistry::new();
    let state = make_state(reg);
    let body = cs_request(json!(1), json!({"I2PTunnel": true}));
    let req: JsonRpcRequest = parse_request(&body.to_string()).unwrap();
    let resp = assemble_response_live(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result["I2PTunnel"]["client"], json!({}));
    assert_eq!(result["I2PTunnel"]["server"], json!({}));
}

#[tokio::test]
async fn i2ptunnel_live_query_reflects_create() {
    // Create a tunnel definition in the FakeTunnelManagerControl
    let tm = FakeTunnelManagerControl::new();
    tm.create(make_tunnel_def(
        "test-tunnel",
        TunnelType::Client,
        Some("abcd.b32.i2p".to_string()),
        None,
        None,
    ))
    .await
    .unwrap();

    let reg = ServiceRegistry::new();
    let mut state = make_state(reg);
    state.set_tunnel_manager(Box::new(tm));

    let body = cs_request(json!(1), json!({"I2PTunnel": true}));
    let req: JsonRpcRequest = parse_request(&body.to_string()).unwrap();
    let resp = assemble_response_live(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    let tunnel = &result["I2PTunnel"];
    assert!(
        tunnel["client"]["test-tunnel"]["address"] == "abcd.b32.i2p",
        "live query must reflect created tunnel: {}",
        tunnel
    );
}

#[tokio::test]
async fn i2ptunnel_live_query_reflects_delete() {
    let tm = FakeTunnelManagerControl::new();
    tm.create(make_tunnel_def(
        "ephemeral",
        TunnelType::Client,
        Some("xyzw.b32.i2p".to_string()),
        None,
        None,
    ))
    .await
    .unwrap();

    let reg = ServiceRegistry::new();
    let mut state = make_state(reg);
    state.set_tunnel_manager(Box::new(tm));

    // Verify it exists
    let body = cs_request(json!(1), json!({"I2PTunnel": true}));
    let req: JsonRpcRequest = parse_request(&body.to_string()).unwrap();
    let resp = assemble_response_live(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    assert!(result["I2PTunnel"]["client"].as_object().unwrap().contains_key("ephemeral"));

    // Delete it
    state.tunnel_manager().delete("ephemeral").await.unwrap();

    // Verify it's gone
    let resp = assemble_response_live(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    assert!(!result["I2PTunnel"]["client"].as_object().unwrap().contains_key("ephemeral"));
}

#[tokio::test]
async fn i2ptunnel_server_tunnel_includes_port() {
    let tm = FakeTunnelManagerControl::new();
    tm.create(make_tunnel_def(
        "my-server",
        TunnelType::Server,
        None,
        Some("host.b32.i2p".to_string()),
        Some(8080),
    ))
    .await
    .unwrap();

    let reg = ServiceRegistry::new();
    let mut state = make_state(reg);
    state.set_tunnel_manager(Box::new(tm));

    let body = cs_request(json!(1), json!({"I2PTunnel": true}));
    let req: JsonRpcRequest = parse_request(&body.to_string()).unwrap();
    let resp = assemble_response_live(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    let server = &result["I2PTunnel"]["server"]["my-server"];
    assert_eq!(server["address"], "host.b32.i2p");
    assert_eq!(server["port"], 8080);
}

// ---------------------------------------------------------------------------
// HTTPProxy/SOCKS listener lifecycle tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn htttpproxy_configured_not_enabled() {
    let reg = ServiceRegistry::new();
    let handle = reg.allocate_handle(ServiceCategory::HttpProxy);
    handle
        .update(
            ObservedServiceState::Configured,
            ServiceMetadata {
                enabled: true,
                ..Default::default()
            },
        )
        .unwrap();
    let state = make_state(reg);
    let body = cs_request(json!(1), json!({"HTTPProxy": true}));
    let req: JsonRpcRequest = parse_request(&body.to_string()).unwrap();
    let resp = assemble_response_live(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    assert_eq!(result["HTTPProxy"]["enabled"], false);
}

#[tokio::test]
async fn htttpproxy_starting_not_enabled() {
    let reg = ServiceRegistry::new();
    let handle = reg.allocate_handle(ServiceCategory::HttpProxy);
    handle
        .update(
            ObservedServiceState::Starting,
            ServiceMetadata {
                enabled: true,
                host: Some("127.0.0.1".into()),
                port: Some(4444),
                ..Default::default()
            },
        )
        .unwrap();
    let state = make_state(reg);
    let body = cs_request(json!(1), json!({"HTTPProxy": true}));
    let req: JsonRpcRequest = parse_request(&body.to_string()).unwrap();
    let resp = assemble_response_live(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    assert_eq!(result["HTTPProxy"]["enabled"], false);
    // address/port should not appear when not listening
    assert!(result["HTTPProxy"].get("address").is_none());
    assert!(result["HTTPProxy"].get("port").is_none());
}

#[tokio::test]
async fn htttpproxy_listening_enabled() {
    let reg = ServiceRegistry::new();
    let handle = reg.allocate_handle(ServiceCategory::HttpProxy);
    handle
        .update(
            ObservedServiceState::Listening,
            ServiceMetadata {
                enabled: true,
                host: Some("127.0.0.1".into()),
                port: Some(4444),
                ..Default::default()
            },
        )
        .unwrap();
    let state = make_state(reg);
    let body = cs_request(json!(1), json!({"HTTPProxy": true}));
    let req: JsonRpcRequest = parse_request(&body.to_string()).unwrap();
    let resp = assemble_response_live(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    assert_eq!(result["HTTPProxy"]["enabled"], true);
    assert_eq!(result["HTTPProxy"]["address"], "127.0.0.1");
    assert_eq!(result["HTTPProxy"]["port"], 4444);
}

#[tokio::test]
async fn socks_starting_not_enabled() {
    let reg = ServiceRegistry::new();
    let handle = reg.allocate_handle(ServiceCategory::Socks);
    handle
        .update(
            ObservedServiceState::Starting,
            ServiceMetadata {
                enabled: true,
                host: Some("127.0.0.1".into()),
                port: Some(1080),
                ..Default::default()
            },
        )
        .unwrap();
    let state = make_state(reg);
    let body = cs_request(json!(1), json!({"SOCKS": true}));
    let req: JsonRpcRequest = parse_request(&body.to_string()).unwrap();
    let resp = assemble_response_live(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    assert_eq!(result["SOCKS"]["enabled"], false);
}

#[tokio::test]
async fn socks_listening_enabled() {
    let reg = ServiceRegistry::new();
    let handle = reg.allocate_handle(ServiceCategory::Socks);
    handle
        .update(
            ObservedServiceState::Listening,
            ServiceMetadata {
                enabled: true,
                host: Some("127.0.0.1".into()),
                port: Some(1080),
                ..Default::default()
            },
        )
        .unwrap();
    let state = make_state(reg);
    let body = cs_request(json!(1), json!({"SOCKS": true}));
    let req: JsonRpcRequest = parse_request(&body.to_string()).unwrap();
    let resp = assemble_response_live(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    assert_eq!(result["SOCKS"]["enabled"], true);
    assert_eq!(result["SOCKS"]["address"], "127.0.0.1");
    assert_eq!(result["SOCKS"]["port"], 1080);
}

#[tokio::test]
async fn proxy_failed_not_enabled() {
    let reg = ServiceRegistry::new();
    let handle = reg.allocate_handle(ServiceCategory::HttpProxy);
    handle
        .update(
            ObservedServiceState::Failed(
                emissary_cli::i2pcontrol::service_registry::SanitizedFailure {
                    error_kind: "ConnectionRefused".to_string(),
                    address: None,
                },
            ),
            ServiceMetadata::default(),
        )
        .unwrap();
    let state = make_state(reg);
    let body = cs_request(json!(1), json!({"HTTPProxy": true}));
    let req: JsonRpcRequest = parse_request(&body.to_string()).unwrap();
    let resp = assemble_response_live(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    assert_eq!(result["HTTPProxy"]["enabled"], false);
}

#[tokio::test]
async fn proxy_stopped_not_enabled() {
    let reg = ServiceRegistry::new();
    let handle = reg.allocate_handle(ServiceCategory::Socks);
    handle
        .update(ObservedServiceState::Stopped, ServiceMetadata::default())
        .unwrap();
    let state = make_state(reg);
    let body = cs_request(json!(1), json!({"SOCKS": true}));
    let req: JsonRpcRequest = parse_request(&body.to_string()).unwrap();
    let resp = assemble_response_live(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    assert_eq!(result["SOCKS"]["enabled"], false);
}

// ---------------------------------------------------------------------------
// SAM listener lifecycle tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sam_disabled_reports_inactive() {
    let reg = ServiceRegistry::new();
    let state = make_state(reg);
    let body = cs_request(json!(1), json!({"SAM": true}));
    let req: JsonRpcRequest = parse_request(&body.to_string()).unwrap();
    let resp = assemble_response_live(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    assert_eq!(result["SAM"]["enabled"], false);
    assert!(result["SAM"]["sessions"].is_object());
}

#[tokio::test]
async fn sam_listening_reports_enabled() {
    let reg = ServiceRegistry::new();
    let handle = reg.allocate_handle(ServiceCategory::Sam);
    handle
        .update(
            ObservedServiceState::Listening,
            ServiceMetadata {
                enabled: true,
                host: Some("127.0.0.1".into()),
                port: Some(7656),
                session_count: Some(0),
                ..Default::default()
            },
        )
        .unwrap();
    let state = make_state(reg);
    let body = cs_request(json!(1), json!({"SAM": true}));
    let req: JsonRpcRequest = parse_request(&body.to_string()).unwrap();
    let resp = assemble_response_live(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    assert_eq!(result["SAM"]["enabled"], true);
    assert!(result["SAM"]["sessions"].is_object());
}

#[tokio::test]
async fn sam_starting_not_enabled() {
    let reg = ServiceRegistry::new();
    let handle = reg.allocate_handle(ServiceCategory::Sam);
    handle
        .update(
            ObservedServiceState::Starting,
            ServiceMetadata {
                enabled: true,
                ..Default::default()
            },
        )
        .unwrap();
    let state = make_state(reg);
    let body = cs_request(json!(1), json!({"SAM": true}));
    let req: JsonRpcRequest = parse_request(&body.to_string()).unwrap();
    let resp = assemble_response_live(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    assert_eq!(result["SAM"]["enabled"], false);
}

// ---------------------------------------------------------------------------
// I2CP listener lifecycle tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn i2cp_listening_reports_enabled() {
    let reg = ServiceRegistry::new();
    let handle = reg.allocate_handle(ServiceCategory::I2cp);
    handle
        .update(
            ObservedServiceState::Listening,
            ServiceMetadata {
                enabled: true,
                ..Default::default()
            },
        )
        .unwrap();
    let state = make_state(reg);
    let body = cs_request(json!(1), json!({"I2CP": true}));
    let req: JsonRpcRequest = parse_request(&body.to_string()).unwrap();
    let resp = assemble_response_live(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    assert_eq!(result["I2CP"]["enabled"], true);
}

#[tokio::test]
async fn i2cp_disabled_reports_inactive() {
    let reg = ServiceRegistry::new();
    let state = make_state(reg);
    let body = cs_request(json!(1), json!({"I2CP": true}));
    let req: JsonRpcRequest = parse_request(&body.to_string()).unwrap();
    let resp = assemble_response_live(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    assert_eq!(result["I2CP"]["enabled"], false);
}

// ---------------------------------------------------------------------------
// BOB always returns false
// ---------------------------------------------------------------------------

#[tokio::test]
async fn bob_always_false() {
    let reg = ServiceRegistry::new();
    let state = make_state(reg);
    let body = cs_request(json!(1), json!({"BOB": true}));
    let req: JsonRpcRequest = parse_request(&body.to_string()).unwrap();
    let resp = assemble_response_live(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    assert_eq!(result["BOB"], false);
}

// ---------------------------------------------------------------------------
// Selector filtering tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn only_requested_selectors_appear() {
    let reg = ServiceRegistry::new();
    let state = make_state(reg);
    let body = cs_request(json!(1), json!({"BOB": true, "I2CP": true}));
    let req: JsonRpcRequest = parse_request(&body.to_string()).unwrap();
    let resp = assemble_response_live(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    assert_eq!(result.len(), 2);
    assert!(result.contains_key("BOB"));
    assert!(result.contains_key("I2CP"));
    assert!(!result.contains_key("HTTPProxy"));
    assert!(!result.contains_key("SOCKS"));
    assert!(!result.contains_key("SAM"));
    assert!(!result.contains_key("I2PTunnel"));
}

#[tokio::test]
async fn empty_selector_returns_empty_result() {
    let reg = ServiceRegistry::new();
    let state = make_state(reg);
    let body = cs_request(json!(1), json!({}));
    let req: JsonRpcRequest = parse_request(&body.to_string()).unwrap();
    let resp = assemble_response_live(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    assert!(result.is_empty());
}

// ---------------------------------------------------------------------------
// Cross-method consistency: create then query
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_tunnel_then_query_visible() {
    let tm = FakeTunnelManagerControl::new();
    let reg = ServiceRegistry::new();
    let mut state = make_state(reg);
    state.set_tunnel_manager(Box::new(tm));

    // Initially empty
    let body = cs_request(json!(1), json!({"I2PTunnel": true}));
    let req: JsonRpcRequest = parse_request(&body.to_string()).unwrap();
    let resp = assemble_response_live(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    assert_eq!(result["I2PTunnel"]["client"], json!({}));

    // Create a tunnel
    state
        .tunnel_manager()
        .create(make_tunnel_def(
            "live-test",
            TunnelType::Client,
            Some("live.b32.i2p".to_string()),
            None,
            None,
        ))
        .await
        .unwrap();

    // Immediately visible
    let resp = assemble_response_live(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    assert!(
        result["I2PTunnel"]["client"]["live-test"]["address"] == "live.b32.i2p",
        "mutation must be visible to next query"
    );

    // Delete
    state.tunnel_manager().delete("live-test").await.unwrap();

    // Gone
    let resp = assemble_response_live(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    assert!(!result["I2PTunnel"]["client"].as_object().unwrap().contains_key("live-test"));
}

// ---------------------------------------------------------------------------
// Restart test: durable definitions survive, volatile state resets
// ---------------------------------------------------------------------------

#[tokio::test]
async fn durable_tunnel_definitions_survive_restart_simulation() {
    // Create definitions in a tunnel manager
    let tm = FakeTunnelManagerControl::new();
    tm.create(make_tunnel_def(
        "durable-client",
        TunnelType::Client,
        Some("durable.b32.i2p".to_string()),
        None,
        None,
    ))
    .await
    .unwrap();
    tm.create(make_tunnel_def(
        "durable-server",
        TunnelType::Server,
        None,
        Some("server.b32.i2p".to_string()),
        Some(9090),
    ))
    .await
    .unwrap();

    // Simulate restart: new registry, new state, but same tunnel manager
    // (in production, the tunnel manager reconstructs from durable store)
    let reg = ServiceRegistry::new();
    let mut state = make_state(reg);
    state.set_tunnel_manager(Box::new(tm));

    let body = cs_request(json!(1), json!({"I2PTunnel": true}));
    let req: JsonRpcRequest = parse_request(&body.to_string()).unwrap();
    let resp = assemble_response_live(&state, &req).await;
    let result = resp["result"].as_object().unwrap();

    // Durable definitions are visible
    assert!(result["I2PTunnel"]["client"]["durable-client"]["address"] == "durable.b32.i2p");
    assert_eq!(
        result["I2PTunnel"]["server"]["durable-server"]["address"],
        "server.b32.i2p"
    );
    assert_eq!(
        result["I2PTunnel"]["server"]["durable-server"]["port"],
        9090
    );
}

// ---------------------------------------------------------------------------
// Stale generation rejection test
// ---------------------------------------------------------------------------

#[test]
fn stale_generation_handle_rejected() {
    let reg = ServiceRegistry::new();
    let stale = reg.allocate_handle(ServiceCategory::HttpProxy);
    let newer = reg.allocate_handle(ServiceCategory::HttpProxy);

    // Stale handle must fail
    let result = stale.update(
        ObservedServiceState::Listening,
        ServiceMetadata {
            enabled: true,
            ..Default::default()
        },
    );
    assert!(result.is_err(), "stale generation must be rejected");

    // Newer handle succeeds
    let result = newer.update(
        ObservedServiceState::Listening,
        ServiceMetadata {
            enabled: true,
            ..Default::default()
        },
    );
    assert!(result.is_ok());
}
