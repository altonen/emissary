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

//! Proposal 170 `ClientServicesInfo` JSON-RPC method handler.
//!
//! Implements the `ClientServicesInfo` method with exact selector-by-presence
//! behavior. Only requested selector keys appear in the response. Each
//! selector's response type follows the Proposal 170 specification exactly.

use std::collections::HashSet;

use emissary_core::{
    SamSessionObservationHandle, SamSessionObservationSnapshot, SAM_SESSION_OBSERVATION_LIMIT,
    SAM_SOCKET_OBSERVATION_LIMIT,
};

use crate::i2pcontrol::{
    control_plane::TunnelManagerControl,
    rpc::{self, JsonRpcErrorResponse, JsonRpcRequest, JsonRpcSuccess, RequestId},
    service_registry::{ObservedServiceState, ServiceCategory, ServiceSnapshot},
};

const LOG_TARGET: &str = "emissary::i2pcontrol::client_services_handler";

/// Valid Proposal 170 ClientServicesInfo selector keys.
const VALID_SELECTORS: &[&str] = &["I2PTunnel", "HTTPProxy", "SOCKS", "SAM", "BOB", "I2CP"];

/// Maximum estimated response bytes (1 MiB).
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

/// Maximum number of tunnel definitions per I2PTunnel response.
const MAX_TUNNEL_DEFINITIONS: usize = 1000;

/// Test if a string is a valid ClientServicesInfo selector key.
pub fn is_valid_client_services_selector(s: &str) -> bool {
    VALID_SELECTORS.contains(&s)
}

/// Estimate worst-case output bytes for the requested selector set.
fn estimate_response_budget(key_set: &HashSet<&str>) -> Result<(), String> {
    let mut estimated_bytes: usize = 0;

    // I2PTunnel: tunnel definitions (each ~200 bytes)
    if key_set.contains("I2PTunnel") {
        estimated_bytes += MAX_TUNNEL_DEFINITIONS * 200;
    }
    // HTTPProxy: small object (~200 bytes)
    if key_set.contains("HTTPProxy") {
        estimated_bytes += 200;
    }
    // SOCKS: small object (~200 bytes)
    if key_set.contains("SOCKS") {
        estimated_bytes += 200;
    }
    // SAM: sessions list (~100 bytes per session)
    if key_set.contains("SAM") {
        estimated_bytes +=
            SAM_SESSION_OBSERVATION_LIMIT * (100 + SAM_SOCKET_OBSERVATION_LIMIT * 80) + 100;
    }
    // BOB: single boolean (~10 bytes)
    if key_set.contains("BOB") {
        estimated_bytes += 10;
    }
    // I2CP: small object (~200 bytes)
    if key_set.contains("I2CP") {
        estimated_bytes += 200;
    }

    if estimated_bytes > MAX_RESPONSE_BYTES {
        return Err(format!(
            "Estimated response size ({estimated_bytes} bytes) exceeds maximum ({MAX_RESPONSE_BYTES} bytes)"
        ));
    }

    Ok(())
}

/// Handle the ClientServicesInfo JSON-RPC method.
///
/// Parses direct Proposal 170 service parameters, dispatches to the service
/// registry snapshot and live tunnel manager, and returns only requested keys.
/// The historical nested `Selector` map remains a compatibility extension.
pub(crate) async fn handle_client_services_info(
    state: &crate::i2pcontrol::server::I2pControlState,
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

    let mut requested_keys: Vec<&str> = Vec::new();
    if let Some(selector) = params.get("Selector") {
        if params.len() != 1 {
            return error_response(
                id,
                rpc::error_codes::INVALID_PARAMS,
                "Direct service parameters cannot be mixed with compatibility 'Selector'",
            );
        }
        let selector_map = match selector {
            serde_json::Value::Object(map) => map,
            _ => {
                return error_response(
                    id,
                    rpc::error_codes::INVALID_PARAMS,
                    "Invalid compatibility 'Selector'; expected a JSON object",
                );
            }
        };
        for (key, value) in selector_map {
            if value.as_bool() == Some(true) {
                if !is_valid_client_services_selector(key) {
                    return error_response(
                        id,
                        rpc::error_codes::INVALID_PARAMS,
                        format!("Unknown selector: '{key}'"),
                    );
                }
                requested_keys.push(key.as_str());
            }
        }
    } else {
        // Canonical Proposal 170 semantics: key presence selects a service,
        // regardless of whether its value is false, null, or another JSON
        // type.
        for key in params.keys() {
            if !is_valid_client_services_selector(key) {
                return error_response(
                    id,
                    rpc::error_codes::INVALID_PARAMS,
                    format!("Unknown direct service parameter: '{key}'"),
                );
            }
            requested_keys.push(key.as_str());
        }
    }

    // Estimate response budget before expensive queries
    let key_set: HashSet<&str> = requested_keys.iter().copied().collect();
    if let Err(e) = estimate_response_budget(&key_set) {
        return error_response(id, rpc::error_codes::INTERNAL_ERROR, e);
    }

    // Take a snapshot from the service registry for listener/proxy state
    let snapshot = state.service_snapshot();

    // Assemble response using live tunnel manager for I2PTunnel
    match assemble_response_with_observation(
        &snapshot,
        &requested_keys,
        state.tunnel_manager(),
        state.sam_session_observation(),
    )
    .await
    {
        Ok(result) => {
            let response = JsonRpcSuccess::new(id, serde_json::Value::Object(result));
            serde_json::to_value(&response).unwrap()
        }
        Err(e) => {
            tracing::error!(target: LOG_TARGET, "ClientServicesInfo assembly failed: {e}");
            error_response(id, rpc::error_codes::INTERNAL_ERROR, e)
        }
    }
}

/// Assemble the response object containing only requested service keys.
///
/// For I2PTunnel, queries the live TunnelManagerControl at request time
/// rather than relying on a startup-only registry snapshot. This ensures
/// Create/Edit/Delete mutations are visible to subsequent queries.
#[allow(dead_code)]
pub async fn assemble_response(
    snapshot: &ServiceSnapshot,
    requested_keys: &[&str],
    tunnel_manager: &dyn TunnelManagerControl,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    assemble_response_with_observation(snapshot, requested_keys, tunnel_manager, None).await
}

/// Assemble a response with the canonical bounded SAM observation source.
pub async fn assemble_response_with_observation(
    snapshot: &ServiceSnapshot,
    requested_keys: &[&str],
    tunnel_manager: &dyn TunnelManagerControl,
    sam_observation: Option<&SamSessionObservationHandle>,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let mut result = serde_json::Map::new();

    if requested_keys.is_empty() {
        return Ok(result);
    }

    for &key in requested_keys {
        let category = match key {
            "I2PTunnel" => ServiceCategory::I2PTunnel,
            "HTTPProxy" => ServiceCategory::HttpProxy,
            "SOCKS" => ServiceCategory::Socks,
            "SAM" => ServiceCategory::Sam,
            "BOB" => ServiceCategory::Bob,
            "I2CP" => ServiceCategory::I2cp,
            _ => {
                return Err(format!("Unknown selector: '{key}'"));
            }
        };

        let entry = snapshot.get(category);

        let value = match category {
            ServiceCategory::I2PTunnel => resolve_i2ptunnel_live(tunnel_manager).await?,
            ServiceCategory::HttpProxy => resolve_httpproxy(entry),
            ServiceCategory::Socks => resolve_socks(entry),
            ServiceCategory::Sam => resolve_sam(entry, sam_observation)?,
            ServiceCategory::Bob => resolve_bob(),
            ServiceCategory::I2cp => resolve_i2cp(entry),
        };

        result.insert(key.to_string(), value);
    }

    Ok(result)
}

/// Resolve I2PTunnel selector by querying the live TunnelManagerControl.
///
/// Per Proposal 170: `{"client": {<name>: {"address": "..."}}, "server": {<name>: {"address":
/// "...", "port": N}}}`
///
/// This queries the shared TunnelManagerControl at request time, ensuring
/// that Create/Edit/Delete mutations are visible without restart. Store
/// failures propagate as errors rather than empty inventory.
async fn resolve_i2ptunnel_live(
    tunnel_manager: &dyn TunnelManagerControl,
) -> Result<serde_json::Value, String> {
    let definitions = tunnel_manager.list().await?;

    let mut client_obj = serde_json::Map::new();
    let mut server_obj = serde_json::Map::new();

    for def in &definitions {
        let name = def.name.as_str().to_string();
        let is_client = def.tunnel_type.is_client();

        let address = def
            .options
            .target_destination
            .clone()
            .or_else(|| def.options.hosting_destination.clone())
            .unwrap_or_default();

        let mut entry = serde_json::Map::new();
        entry.insert("address".to_string(), serde_json::json!(address));

        if is_client {
            client_obj.insert(name, serde_json::Value::Object(entry));
        } else {
            if let Some(port) = def.options.listen_port {
                entry.insert("port".to_string(), serde_json::json!(port));
            }
            server_obj.insert(name, serde_json::Value::Object(entry));
        }
    }

    Ok(serde_json::json!({
        "client": serde_json::Value::Object(client_obj),
        "server": serde_json::Value::Object(server_obj),
    }))
}

/// Resolve HTTPProxy selector.
///
/// Per Proposal 170: `{"enabled": bool, "address": "...", "port": N}`
///
/// `enabled: true` only after a successful bind (`Listening` state).
/// `Configured` and `Starting` report `enabled: false` because no
/// listener has actually bound yet.
fn resolve_httpproxy(
    entry: Option<&crate::i2pcontrol::service_registry::ServiceEntry>,
) -> serde_json::Value {
    let entry = match entry {
        Some(e) => e,
        None => {
            return serde_json::json!({
                "enabled": false
            });
        }
    };

    match &entry.state {
        ObservedServiceState::Disabled => serde_json::json!({
            "enabled": false
        }),
        ObservedServiceState::Configured | ObservedServiceState::Starting => {
            // Not yet listening — report disabled even if configured
            serde_json::json!({
                "enabled": false,
            })
        }
        ObservedServiceState::Listening => serde_json::json!({
            "enabled": true,
            "address": entry.metadata.host,
            "port": entry.metadata.port,
        }),
        ObservedServiceState::Failed(_) => serde_json::json!({
            "enabled": false,
        }),
        ObservedServiceState::Stopping | ObservedServiceState::Stopped => serde_json::json!({
            "enabled": false,
        }),
    }
}

/// Resolve SOCKS selector.
///
/// Per Proposal 170: `{"enabled": bool, "address": "...", "port": N}`
///
/// `enabled: true` only after a successful bind (`Listening` state).
/// `Configured` and `Starting` report `enabled: false` because no
/// listener has actually bound yet.
fn resolve_socks(
    entry: Option<&crate::i2pcontrol::service_registry::ServiceEntry>,
) -> serde_json::Value {
    let entry = match entry {
        Some(e) => e,
        None => {
            return serde_json::json!({
                "enabled": false
            });
        }
    };

    match &entry.state {
        ObservedServiceState::Disabled => serde_json::json!({
            "enabled": false
        }),
        ObservedServiceState::Configured | ObservedServiceState::Starting => {
            // Not yet listening — report disabled even if configured
            serde_json::json!({
                "enabled": false,
            })
        }
        ObservedServiceState::Listening => serde_json::json!({
            "enabled": true,
            "address": entry.metadata.host,
            "port": entry.metadata.port,
        }),
        ObservedServiceState::Failed(_) => serde_json::json!({
            "enabled": false,
        }),
        ObservedServiceState::Stopping | ObservedServiceState::Stopped => serde_json::json!({
            "enabled": false,
        }),
    }
}

/// Resolve SAM selector.
///
/// Per Proposal 170: `{"enabled": bool, "sessions": {}}`.
///
/// `enabled: true` only when the SAM listener is actively bound.
/// `Configured` and `Starting` report `enabled: false`.
fn resolve_sam(
    entry: Option<&crate::i2pcontrol::service_registry::ServiceEntry>,
    sam_observation: Option<&SamSessionObservationHandle>,
) -> Result<serde_json::Value, String> {
    let entry = match entry {
        Some(e) => e,
        None => {
            return Ok(serde_json::json!({
                "enabled": false,
                "sessions": {}
            }));
        }
    };

    match &entry.state {
        ObservedServiceState::Disabled => Ok(serde_json::json!({
            "enabled": false,
            "sessions": {}
        })),
        ObservedServiceState::Configured | ObservedServiceState::Starting => {
            // Not yet listening — report disabled even if configured
            Ok(serde_json::json!({
                "enabled": false,
                "sessions": {}
            }))
        }
        ObservedServiceState::Listening => {
            let observation = sam_observation.ok_or_else(|| {
                "SAM listener is active but its canonical observation source is unavailable"
                    .to_string()
            })?;
            let snapshot = observation.snapshot().map_err(|_| {
                "SAM observation source overflowed; refusing an incomplete snapshot".to_string()
            })?;
            if snapshot.sessions.len() > SAM_SESSION_OBSERVATION_LIMIT {
                return Err("SAM observation source exceeded its session bound".to_string());
            }
            let sessions = serialize_sam_sessions(snapshot)?;
            Ok(serde_json::json!({
                "enabled": true,
                "sessions": sessions
            }))
        }
        ObservedServiceState::Failed(_) => Ok(serde_json::json!({
            "enabled": false,
            "sessions": {}
        })),
        ObservedServiceState::Stopping | ObservedServiceState::Stopped => Ok(serde_json::json!({
            "enabled": false,
            "sessions": {}
        })),
    }
}

fn serialize_sam_sessions(
    snapshot: SamSessionObservationSnapshot,
) -> Result<serde_json::Value, String> {
    if snapshot.sessions.len() > SAM_SESSION_OBSERVATION_LIMIT {
        return Err("SAM observation source exceeded its session bound".to_string());
    }

    let sessions = snapshot
        .sessions
        .into_iter()
        .map(|(session_id, session)| {
            (
                session_id.to_string(),
                serde_json::json!({
                    "name": session.name.as_ref(),
                    "address": session.address.as_ref(),
                    "sockets": session.sockets.into_iter().map(|socket| serde_json::json!({
                        "type": socket.socket_type,
                        "peer": socket.peer.as_ref(),
                    })).collect::<Vec<_>>(),
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();

    Ok(serde_json::Value::Object(sessions))
}

/// Resolve BOB selector.
///
/// Per Proposal 170: BOB returns `false` (not implemented in Emissary).
/// No BOB listener, stub server, or configuration is added.
fn resolve_bob() -> serde_json::Value {
    serde_json::json!(false)
}

/// Resolve I2CP selector.
///
/// Per Proposal 170: `{"enabled": bool}`
///
/// `enabled: true` only while the I2CP listener is actively bound.
/// `Configured` and `Starting` report `enabled: false` because no
/// listener has actually bound yet.
fn resolve_i2cp(
    entry: Option<&crate::i2pcontrol::service_registry::ServiceEntry>,
) -> serde_json::Value {
    let entry = match entry {
        Some(e) => e,
        None => {
            return serde_json::json!({
                "enabled": false
            });
        }
    };

    match &entry.state {
        ObservedServiceState::Disabled => serde_json::json!({
            "enabled": false
        }),
        ObservedServiceState::Configured | ObservedServiceState::Starting => {
            // Not yet listening — report disabled even if configured
            serde_json::json!({
                "enabled": false,
            })
        }
        ObservedServiceState::Listening => serde_json::json!({
            "enabled": true,
        }),
        ObservedServiceState::Failed(_) => serde_json::json!({
            "enabled": false,
        }),
        ObservedServiceState::Stopping | ObservedServiceState::Stopped => serde_json::json!({
            "enabled": false,
        }),
    }
}

fn resolve_id(id: &Option<RequestId>) -> RequestId {
    id.clone().unwrap_or(RequestId::Null)
}

fn error_response(id: RequestId, code: i32, message: impl Into<String>) -> serde_json::Value {
    serde_json::to_value(JsonRpcErrorResponse::new(id, code, message)).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i2pcontrol::{
        rpc::JsonRpcRequest,
        service_registry::{ServiceCategory, ServiceMetadata, ServiceRegistry},
    };

    fn test_request(selectors: serde_json::Value) -> JsonRpcRequest {
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "ClientServicesInfo".to_string(),
            params: Some(serde_json::json!({"Selector": selectors}).as_object().cloned().unwrap()),
            id: Some(rpc::RequestId::Number(1)),
        }
    }

    fn direct_request(params: serde_json::Value) -> JsonRpcRequest {
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "ClientServicesInfo".to_string(),
            params: Some(params.as_object().cloned().unwrap()),
            id: Some(rpc::RequestId::Number(1)),
        }
    }

    fn test_state(reg: ServiceRegistry) -> crate::i2pcontrol::server::I2pControlState {
        let mut state = crate::i2pcontrol::server::I2pControlState::new_test("test".to_string());
        state.set_service_registry(reg);
        state
    }

    #[tokio::test]
    async fn handle_empty_selector() {
        let reg = ServiceRegistry::new();
        let state = test_state(reg);
        let req = test_request(serde_json::json!({}));
        let resp = handle_client_services_info(&state, &req).await;
        assert_eq!(resp["jsonrpc"], "2.0");
        assert!(resp["result"].is_object());
        assert_eq!(resp["result"].as_object().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn handle_bob_selector() {
        let reg = ServiceRegistry::new();
        let state = test_state(reg);
        let req = test_request(serde_json::json!({"BOB": true}));
        let resp = handle_client_services_info(&state, &req).await;
        let result = resp["result"].as_object().unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result["BOB"], false);
    }

    #[tokio::test]
    async fn canonical_direct_wire_fixture_selects_by_presence() {
        let reg = ServiceRegistry::new();
        let state = test_state(reg);
        let req = direct_request(serde_json::json!({
            "I2PTunnel": "ignored",
            "SAM": false,
        }));
        let resp = handle_client_services_info(&state, &req).await;
        let result = resp["result"].as_object().unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.contains_key("I2PTunnel"));
        assert!(result.contains_key("SAM"));
    }

    #[tokio::test]
    async fn canonical_and_compatibility_selectors_cannot_be_mixed() {
        let state = test_state(ServiceRegistry::new());
        let req = direct_request(serde_json::json!({
            "Selector": {"BOB": true},
            "SAM": true,
        }));
        let resp = handle_client_services_info(&state, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn handle_httpproxy_disabled() {
        let reg = ServiceRegistry::new();
        let state = test_state(reg);
        let req = test_request(serde_json::json!({"HTTPProxy": true}));
        let resp = handle_client_services_info(&state, &req).await;
        let result = resp["result"].as_object().unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result["HTTPProxy"]["enabled"], false);
    }

    #[tokio::test]
    async fn handle_httpproxy_listening() {
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
        let state = test_state(reg);
        let req = test_request(serde_json::json!({"HTTPProxy": true}));
        let resp = handle_client_services_info(&state, &req).await;
        let result = resp["result"].as_object().unwrap();
        assert_eq!(result["HTTPProxy"]["enabled"], true);
        assert_eq!(result["HTTPProxy"]["address"], "127.0.0.1");
        assert_eq!(result["HTTPProxy"]["port"], 4444);
    }

    #[tokio::test]
    async fn handle_socks_listening() {
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
        let state = test_state(reg);
        let req = test_request(serde_json::json!({"SOCKS": true}));
        let resp = handle_client_services_info(&state, &req).await;
        let result = resp["result"].as_object().unwrap();
        assert_eq!(result["SOCKS"]["enabled"], true);
        assert_eq!(result["SOCKS"]["address"], "127.0.0.1");
        assert_eq!(result["SOCKS"]["port"], 1080);
    }

    #[tokio::test]
    async fn handle_sam_disabled() {
        let reg = ServiceRegistry::new();
        let state = test_state(reg);
        let req = test_request(serde_json::json!({"SAM": true}));
        let resp = handle_client_services_info(&state, &req).await;
        let result = resp["result"].as_object().unwrap();
        assert_eq!(result["SAM"]["enabled"], false);
        assert!(result["SAM"]["sessions"].is_object());
    }

    #[tokio::test]
    async fn handle_sam_listening() {
        let reg = ServiceRegistry::new();
        let handle = reg.allocate_handle(ServiceCategory::Sam);
        handle
            .update(
                ObservedServiceState::Listening,
                ServiceMetadata {
                    enabled: true,
                    host: Some("127.0.0.1".into()),
                    port: Some(7656),
                    session_count: Some(2),
                    ..Default::default()
                },
            )
            .unwrap();
        let state = test_state(reg);
        let req = test_request(serde_json::json!({"SAM": true}));
        let resp = handle_client_services_info(&state, &req).await;
        let result = resp["result"].as_object().unwrap();
        assert_eq!(result["SAM"]["enabled"], true);
    }

    #[tokio::test]
    async fn handle_i2cp_disabled() {
        let reg = ServiceRegistry::new();
        let state = test_state(reg);
        let req = test_request(serde_json::json!({"I2CP": true}));
        let resp = handle_client_services_info(&state, &req).await;
        let result = resp["result"].as_object().unwrap();
        assert_eq!(result["I2CP"]["enabled"], false);
    }

    #[tokio::test]
    async fn handle_i2cp_listening() {
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
        let state = test_state(reg);
        let req = test_request(serde_json::json!({"I2CP": true}));
        let resp = handle_client_services_info(&state, &req).await;
        let result = resp["result"].as_object().unwrap();
        assert_eq!(result["I2CP"]["enabled"], true);
    }

    #[tokio::test]
    async fn handle_multiple_selectors() {
        let reg = ServiceRegistry::new();
        let h_http = reg.allocate_handle(ServiceCategory::HttpProxy);
        h_http
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
        let state = test_state(reg);
        let req = test_request(serde_json::json!({
            "HTTPProxy": true,
            "BOB": true,
            "I2CP": true
        }));
        let resp = handle_client_services_info(&state, &req).await;
        let result = resp["result"].as_object().unwrap();
        assert_eq!(result.len(), 3);
        assert!(result.contains_key("HTTPProxy"));
        assert!(result.contains_key("BOB"));
        assert!(result.contains_key("I2CP"));
        // SOCKS not requested, should not appear
        assert!(!result.contains_key("SOCKS"));
        assert!(!result.contains_key("SAM"));
        assert!(!result.contains_key("I2PTunnel"));
    }

    #[tokio::test]
    async fn handle_unknown_selector() {
        let reg = ServiceRegistry::new();
        let state = test_state(reg);
        let req = test_request(serde_json::json!({"Unknown": true}));
        let resp = handle_client_services_info(&state, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn handle_false_selector_ignored() {
        let reg = ServiceRegistry::new();
        let state = test_state(reg);
        let req = test_request(serde_json::json!({"BOB": false}));
        let resp = handle_client_services_info(&state, &req).await;
        let result = resp["result"].as_object().unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn handle_missing_selector_param() {
        let reg = ServiceRegistry::new();
        let state = test_state(reg);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "ClientServicesInfo".to_string(),
            params: Some(serde_json::json!({"Token": "abc"}).as_object().cloned().unwrap()),
            id: Some(rpc::RequestId::Number(1)),
        };
        let resp = handle_client_services_info(&state, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn handle_no_params() {
        let reg = ServiceRegistry::new();
        let state = test_state(reg);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "ClientServicesInfo".to_string(),
            params: None,
            id: Some(rpc::RequestId::Number(1)),
        };
        let resp = handle_client_services_info(&state, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    #[test]
    fn validate_selector_keys() {
        assert!(is_valid_client_services_selector("I2PTunnel"));
        assert!(is_valid_client_services_selector("HTTPProxy"));
        assert!(is_valid_client_services_selector("SOCKS"));
        assert!(is_valid_client_services_selector("SAM"));
        assert!(is_valid_client_services_selector("BOB"));
        assert!(is_valid_client_services_selector("I2CP"));
        assert!(!is_valid_client_services_selector("unknown"));
        assert!(!is_valid_client_services_selector(""));
    }

    #[test]
    fn resolve_bob_returns_false() {
        let value = resolve_bob();
        assert_eq!(value, serde_json::json!(false));
    }

    #[tokio::test]
    async fn resolve_i2ptunnel_live_empty_when_no_definitions() {
        use crate::i2pcontrol::control_plane::FakeTunnelManagerControl;
        let tm = FakeTunnelManagerControl::new();
        let value = resolve_i2ptunnel_live(&tm).await.unwrap();
        assert_eq!(value["client"], serde_json::json!({}));
        assert_eq!(value["server"], serde_json::json!({}));
    }

    #[test]
    fn resolve_httpproxy_disabled_when_none() {
        let value = resolve_httpproxy(None);
        assert_eq!(value["enabled"], false);
    }

    #[test]
    fn resolve_httpproxy_configured_not_enabled() {
        let entry = crate::i2pcontrol::service_registry::ServiceEntry {
            category: ServiceCategory::HttpProxy,
            state: ObservedServiceState::Configured,
            metadata: ServiceMetadata {
                enabled: true,
                ..Default::default()
            },
        };
        let value = resolve_httpproxy(Some(&entry));
        assert_eq!(value["enabled"], false);
    }

    #[test]
    fn resolve_httpproxy_starting_not_enabled() {
        let entry = crate::i2pcontrol::service_registry::ServiceEntry {
            category: ServiceCategory::HttpProxy,
            state: ObservedServiceState::Starting,
            metadata: ServiceMetadata {
                enabled: true,
                host: Some("127.0.0.1".into()),
                port: Some(4444),
                ..Default::default()
            },
        };
        let value = resolve_httpproxy(Some(&entry));
        assert_eq!(value["enabled"], false);
        // address/port should not be present when not listening
        assert!(value.get("address").is_none());
        assert!(value.get("port").is_none());
    }

    #[test]
    fn resolve_socks_disabled_when_none() {
        let value = resolve_socks(None);
        assert_eq!(value["enabled"], false);
    }

    #[test]
    fn resolve_socks_starting_not_enabled() {
        let entry = crate::i2pcontrol::service_registry::ServiceEntry {
            category: ServiceCategory::Socks,
            state: ObservedServiceState::Starting,
            metadata: ServiceMetadata {
                enabled: true,
                host: Some("127.0.0.1".into()),
                port: Some(1080),
                ..Default::default()
            },
        };
        let value = resolve_socks(Some(&entry));
        assert_eq!(value["enabled"], false);
    }

    #[test]
    fn resolve_sam_disabled_when_none() {
        let value = resolve_sam(None, None).unwrap();
        assert_eq!(value["enabled"], false);
        assert!(value["sessions"].is_object());
    }

    #[test]
    fn resolve_sam_configured_not_enabled() {
        let entry = crate::i2pcontrol::service_registry::ServiceEntry {
            category: ServiceCategory::Sam,
            state: ObservedServiceState::Configured,
            metadata: ServiceMetadata {
                enabled: true,
                ..Default::default()
            },
        };
        let value = resolve_sam(Some(&entry), None).unwrap();
        assert_eq!(value["enabled"], false);
        assert!(value["sessions"].is_object());
    }

    #[test]
    fn resolve_sam_starting_not_enabled() {
        let entry = crate::i2pcontrol::service_registry::ServiceEntry {
            category: ServiceCategory::Sam,
            state: ObservedServiceState::Starting,
            metadata: ServiceMetadata {
                enabled: true,
                ..Default::default()
            },
        };
        let value = resolve_sam(Some(&entry), None).unwrap();
        assert_eq!(value["enabled"], false);
    }

    #[test]
    fn resolve_sam_listening_without_observation_is_unavailable() {
        let entry = crate::i2pcontrol::service_registry::ServiceEntry {
            category: ServiceCategory::Sam,
            state: ObservedServiceState::Listening,
            metadata: ServiceMetadata {
                enabled: true,
                ..Default::default()
            },
        };
        let error = resolve_sam(Some(&entry), None).unwrap_err();
        assert!(error.contains("canonical observation source is unavailable"));
    }

    #[test]
    fn serialize_sam_sessions_preserves_pinned_active_shape() {
        use std::{collections::BTreeMap, sync::Arc};

        let snapshot = SamSessionObservationSnapshot {
            sessions: BTreeMap::from([(
                Arc::from("chat"),
                emissary_core::SamObservedSession {
                    name: Arc::from("chat"),
                    address: Arc::from("chat.b32.i2p"),
                    sockets: vec![emissary_core::SamObservedSocket {
                        socket_type: 2,
                        peer: Arc::from("127.0.0.1:7656"),
                    }],
                },
            )]),
            generation: 1,
        };

        let value = serialize_sam_sessions(snapshot).unwrap();
        assert_eq!(value["chat"]["name"], "chat");
        assert_eq!(value["chat"]["address"], "chat.b32.i2p");
        assert_eq!(value["chat"]["sockets"][0]["type"], 2);
        assert_eq!(value["chat"]["sockets"][0]["peer"], "127.0.0.1:7656");
    }

    #[test]
    fn resolve_i2cp_disabled_when_none() {
        let value = resolve_i2cp(None);
        assert_eq!(value["enabled"], false);
    }

    #[test]
    fn resolve_i2cp_starting_not_enabled() {
        let entry = crate::i2pcontrol::service_registry::ServiceEntry {
            category: ServiceCategory::I2cp,
            state: ObservedServiceState::Starting,
            metadata: ServiceMetadata {
                enabled: true,
                ..Default::default()
            },
        };
        let value = resolve_i2cp(Some(&entry));
        assert_eq!(value["enabled"], false);
    }

    #[test]
    fn budget_estimation_within_bounds() {
        let mut key_set = HashSet::new();
        key_set.insert("BOB");
        assert!(estimate_response_budget(&key_set).is_ok());
    }
}
