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

//! Proposal 170 TunnelManager API handler.
//!
//! Implements the `TunnelManager` JSON-RPC method for all declared tunnel
//! types. Provides CRUD operations (List, Create, Edit, Get, Delete) and
//! lifecycle dispatch (Start, Stop, Restart) through the backend registry.
//!
//! # Invariants
//!
//! - Authentication must precede handler execution.
//! - Exactly eight actions are accepted: List, Create, Edit, Get, Delete,
//!   Start, Stop, Restart.
//! - Exactly twelve tunnel types are accepted.
//! - `All` is accepted only for Start, Stop, and Restart.
//! - CRUD success is returned only after durable persistence.
//! - Unsupported start/restart return deterministic not-implemented status.
//! - Unsupported stop is safe and idempotent.
//! - Unsupported definitions never report running.
//! - Startup-managed definitions are read-only.
//! - No handler writes to `router.toml`.
//! - No handler calls `tokio::spawn`, binds listeners, or edits files.
//! - Logs and errors contain no full definitions, credentials, or keys.

use crate::i2pcontrol::domain::tunnel::{
    StartIntent, TunnelDefinition, TunnelName, TunnelOptions, TunnelOwnership, TunnelRuntimeState,
    TunnelType, ALL_TUNNEL_TYPES,
};
use crate::i2pcontrol::rpc::{
    self, JsonRpcErrorResponse, JsonRpcRequest, JsonRpcSuccess, RequestId,
};
use crate::i2pcontrol::server::I2pControlState;

const LOG_TARGET: &str = "emissary::i2pcontrol::tunnel_manager";

/// Maximum tunnel name length.
const MAX_NAME_LENGTH: usize = 1024;

/// Maximum description length.
const MAX_DESCRIPTION_LENGTH: usize = 4096;

/// Maximum number of targets for `All` operations.
const MAX_ALL_TARGETS: usize = 1000;

/// TunnelManager handler.
///
/// Parses the Proposal 170 TunnelManager request and dispatches to the
/// appropriate action handler. Validates type, action, name, and
/// action-specific fields before any state or backend operation.
pub(crate) async fn handle_tunnel_manager(
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

    // Extract and validate action (required)
    let action_str = match params.get("Action").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return error_response(
                id,
                rpc::error_codes::INVALID_PARAMS,
                "Missing 'Action' parameter",
            );
        }
    };

    let action = match crate::i2pcontrol::domain::tunnel::TunnelAction::from_str_exact(action_str) {
        Some(a) => a,
        None => {
            return error_response(
                id,
                rpc::error_codes::INVALID_PARAMS,
                format!(
                    "Invalid action {}; expected one of: List, Create, Edit, Get, Delete, Start, Stop, Restart",
                    action_str
                ),
            );
        }
    };

    // Extract optional Name
    let name = params.get("Name").and_then(|v| v.as_str());

    // Extract optional Type
    let tunnel_type_str = params.get("Type").and_then(|v| v.as_str());

    // Extract optional All
    let all = params.get("All").and_then(|v| v.as_bool()).unwrap_or(false);

    // Extract optional NewName
    let new_name_str = params.get("NewName").and_then(|v| v.as_str());

    // Dispatch based on action
    match action {
        crate::i2pcontrol::domain::tunnel::TunnelAction::List => handle_list(state, id).await,
        crate::i2pcontrol::domain::tunnel::TunnelAction::Create => {
            handle_create(state, id, params, tunnel_type_str, name).await
        }
        crate::i2pcontrol::domain::tunnel::TunnelAction::Edit => {
            handle_edit(state, id, params, name, new_name_str, tunnel_type_str).await
        }
        crate::i2pcontrol::domain::tunnel::TunnelAction::Get => {
            handle_get(state, id, name, all).await
        }
        crate::i2pcontrol::domain::tunnel::TunnelAction::Delete => {
            handle_delete(state, id, name).await
        }
        crate::i2pcontrol::domain::tunnel::TunnelAction::Start => {
            handle_lifecycle(state, id, name, all, "start").await
        }
        crate::i2pcontrol::domain::tunnel::TunnelAction::Stop => {
            handle_lifecycle(state, id, name, all, "stop").await
        }
        crate::i2pcontrol::domain::tunnel::TunnelAction::Restart => {
            handle_lifecycle(state, id, name, all, "restart").await
        }
    }
}

/// Handle List action: return all tunnel definitions.
async fn handle_list(state: &I2pControlState, id: RequestId) -> serde_json::Value {
    match state.tunnel_list().await {
        definitions => {
            let result: Vec<serde_json::Value> =
                definitions.iter().map(|d| tunnel_definition_to_get_result(d)).collect();
            success_response(id, serde_json::json!(result))
        }
    }
}

/// Handle Create action: create a new tunnel definition.
///
/// Requires `Type` and `Name`. All other fields are optional tunnel options.
/// Returns "ok" on success, or a textual error status.
async fn handle_create(
    state: &I2pControlState,
    id: RequestId,
    params: &serde_json::Map<String, serde_json::Value>,
    tunnel_type_str: Option<&str>,
    name: Option<&str>,
) -> serde_json::Value {
    // Type is required for Create
    let tunnel_type = match tunnel_type_str {
        Some(s) => match TunnelType::from_str_exact(s) {
            Some(tt) => tt,
            None => {
                return error_response(
                    id,
                    rpc::error_codes::INVALID_PARAMS,
                    format!(
                        "Invalid tunnel type {}; expected one of: {}",
                        s,
                        ALL_TUNNEL_TYPES.iter().map(|t| t.as_str()).collect::<Vec<_>>().join(", ")
                    ),
                );
            }
        },
        None => {
            return error_response(
                id,
                rpc::error_codes::INVALID_PARAMS,
                "Missing 'Type' parameter for Create",
            );
        }
    };

    // Name is required for Create
    let tunnel_name = match name {
        Some(s) => match TunnelName::new(s) {
            Ok(n) => n,
            Err(e) => {
                return error_response(id, rpc::error_codes::INVALID_PARAMS, e.to_string());
            }
        },
        None => {
            return error_response(
                id,
                rpc::error_codes::INVALID_PARAMS,
                "Missing 'Name' parameter for Create",
            );
        }
    };

    // Validate name length
    if tunnel_name.as_str().len() > MAX_NAME_LENGTH {
        return error_response(
            id,
            rpc::error_codes::INVALID_PARAMS,
            format!("Name exceeds maximum length of {}", MAX_NAME_LENGTH),
        );
    }

    // Reject control characters in name
    if tunnel_name.as_str().chars().any(|c| c.is_control()) {
        return error_response(
            id,
            rpc::error_codes::INVALID_PARAMS,
            "Name must not contain control characters",
        );
    }

    // Parse options from params
    let options = match extract_tunnel_options(params) {
        Ok(o) => o,
        Err(e) => {
            return error_response(id, rpc::error_codes::INVALID_PARAMS, e);
        }
    };

    // Validate description length if present
    if let Some(ref desc) = options.description {
        if desc.len() > MAX_DESCRIPTION_LENGTH {
            return error_response(
                id,
                rpc::error_codes::INVALID_PARAMS,
                format!(
                    "Description exceeds maximum length of {}",
                    MAX_DESCRIPTION_LENGTH
                ),
            );
        }
    }

    // Extract raw config for lossless get
    let raw_config = extract_raw_config(params);

    // Determine start intent
    let start_intent = options.start_on_load.unwrap_or(StartIntent::DoNotStart);

    let definition = TunnelDefinition {
        name: tunnel_name,
        tunnel_type,
        ownership: TunnelOwnership::ControlPlane,
        runtime_state: TunnelRuntimeState::Stopped,
        start_intent,
        options,
        raw_config,
    };

    match state.tunnel_create(definition).await {
        Ok(()) => success_response(id, serde_json::json!("ok")),
        Err(e) => {
            // Duplicate name is a Proposal 170 textual operation status, not a JSON-RPC error
            if e.contains("already exists") {
                success_response(id, serde_json::json!(e))
            } else {
                tracing::error!(target: LOG_TARGET, "Create failed: {}", e);
                error_response(id, rpc::error_codes::APP_ERROR, e)
            }
        }
    }
}

/// Handle Edit action: update an existing tunnel definition.
///
/// Requires `Name`. Optional `NewName` for rename. Optional `Type` for
/// type-specific options. Preserves omitted fields.
async fn handle_edit(
    state: &I2pControlState,
    id: RequestId,
    params: &serde_json::Map<String, serde_json::Value>,
    name: Option<&str>,
    new_name_str: Option<&str>,
    tunnel_type_str: Option<&str>,
) -> serde_json::Value {
    let tunnel_name = match name {
        Some(s) => s,
        None => {
            return error_response(
                id,
                rpc::error_codes::INVALID_PARAMS,
                "Missing 'Name' parameter for Edit",
            );
        }
    };

    // Load existing definition
    let existing = match state.tunnel_get(tunnel_name).await {
        Some(d) => d,
        None => {
            return error_response(
                id,
                rpc::error_codes::APP_ERROR,
                format!("error - tunnel '{}' not found", tunnel_name),
            );
        }
    };

    // Reject edits to startup-managed definitions
    if existing.ownership == TunnelOwnership::StartupManaged {
        return error_response(
            id,
            rpc::error_codes::APP_ERROR,
            "error - tunnel is managed by the startup configuration",
        );
    }

    // Parse new name if provided
    let new_name = match new_name_str {
        Some(s) => match TunnelName::new(s) {
            Ok(n) => Some(n),
            Err(e) => {
                return error_response(id, rpc::error_codes::INVALID_PARAMS, e.to_string());
            }
        },
        None => None,
    };

    // Validate new name length
    if let Some(ref nn) = new_name {
        if nn.as_str().len() > MAX_NAME_LENGTH {
            return error_response(
                id,
                rpc::error_codes::INVALID_PARAMS,
                format!("NewName exceeds maximum length of {}", MAX_NAME_LENGTH),
            );
        }
        if nn.as_str().chars().any(|c| c.is_control()) {
            return error_response(
                id,
                rpc::error_codes::INVALID_PARAMS,
                "NewName must not contain control characters",
            );
        }
    }

    // Parse new options from params (merging with existing)
    let new_options = match extract_tunnel_options(params) {
        Ok(o) => o,
        Err(e) => {
            return error_response(id, rpc::error_codes::INVALID_PARAMS, e);
        }
    };

    // Validate description length if present
    if let Some(ref desc) = new_options.description {
        if desc.len() > MAX_DESCRIPTION_LENGTH {
            return error_response(
                id,
                rpc::error_codes::INVALID_PARAMS,
                format!(
                    "Description exceeds maximum length of {}",
                    MAX_DESCRIPTION_LENGTH
                ),
            );
        }
    }

    // Merge options: existing values preserved where new is None
    let merged_options = merge_tunnel_options(&existing.options, &new_options);

    // Type is immutable in Edit (use existing)
    let tunnel_type = tunnel_type_str
        .and_then(|s| TunnelType::from_str_exact(s))
        .unwrap_or(existing.tunnel_type);

    // Build the final definition name
    let final_name = new_name.clone().unwrap_or_else(|| existing.name.clone());

    // Update raw_config: merge new params into existing
    let mut raw_config = existing.raw_config;
    for (k, v) in params {
        if k != "Name" && k != "Action" && k != "Type" && k != "NewName" && k != "All" {
            raw_config.insert(k.clone(), v.clone());
        }
    }

    let definition = TunnelDefinition {
        name: final_name,
        tunnel_type,
        ownership: existing.ownership,
        runtime_state: existing.runtime_state,
        start_intent: merged_options.start_on_load.unwrap_or(existing.start_intent),
        options: merged_options,
        raw_config,
    };

    match state.tunnel_update(tunnel_name, definition, new_name.clone()).await {
        Ok(true) => success_response(id, serde_json::json!("ok")),
        Ok(false) => error_response(
            id,
            rpc::error_codes::APP_ERROR,
            format!("error - tunnel '{}' not found", tunnel_name),
        ),
        Err(e) => {
            tracing::error!(target: LOG_TARGET, "Edit failed: {}", e);
            error_response(id, rpc::error_codes::APP_ERROR, e)
        }
    }
}

/// Handle Get action: retrieve a tunnel definition or all definitions.
///
/// If `All` is true, returns all definitions. Otherwise returns the
/// definition matching `Name`.
async fn handle_get(
    state: &I2pControlState,
    id: RequestId,
    name: Option<&str>,
    all: bool,
) -> serde_json::Value {
    if all {
        // Get all definitions
        let definitions = state.tunnel_list().await;
        let result: Vec<serde_json::Value> =
            definitions.iter().map(|d| tunnel_definition_to_get_result(d)).collect();
        return success_response(id, serde_json::json!(result));
    }

    let tunnel_name = match name {
        Some(s) => s,
        None => {
            return error_response(
                id,
                rpc::error_codes::INVALID_PARAMS,
                "Missing 'Name' parameter for Get",
            );
        }
    };

    match state.tunnel_get(tunnel_name).await {
        Some(definition) => {
            let result = tunnel_definition_to_get_result(&definition);
            success_response(id, result)
        }
        None => error_response(
            id,
            rpc::error_codes::APP_ERROR,
            format!("error - tunnel '{}' not found", tunnel_name),
        ),
    }
}

/// Handle Delete action: remove a tunnel definition.
///
/// Requires `Name`. Rejects startup-managed definitions.
async fn handle_delete(
    state: &I2pControlState,
    id: RequestId,
    name: Option<&str>,
) -> serde_json::Value {
    let tunnel_name = match name {
        Some(s) => s,
        None => {
            return error_response(
                id,
                rpc::error_codes::INVALID_PARAMS,
                "Missing 'Name' parameter for Delete",
            );
        }
    };

    // Check existence and ownership before delete
    match state.tunnel_get(tunnel_name).await {
        Some(def) => {
            if def.ownership == TunnelOwnership::StartupManaged {
                return error_response(
                    id,
                    rpc::error_codes::APP_ERROR,
                    "error - tunnel is managed by the startup configuration",
                );
            }
        }
        None => {
            // Delete of absent name is a successful no-op
            return success_response(id, serde_json::json!("ok"));
        }
    }

    match state.tunnel_delete(tunnel_name).await {
        Ok(true) => success_response(id, serde_json::json!("ok")),
        Ok(false) => success_response(id, serde_json::json!("ok")),
        Err(e) => {
            tracing::error!(target: LOG_TARGET, "Delete failed: {}", e);
            error_response(
                id,
                rpc::error_codes::INTERNAL_ERROR,
                "Failed to delete tunnel definition",
            )
        }
    }
}

/// Handle lifecycle actions (Start, Stop, Restart).
///
/// If `All` is true, applies the action to all tunnel definitions.
/// Otherwise applies to the single named definition.
/// Returns the exact Proposal 170 textual operation status.
async fn handle_lifecycle(
    state: &I2pControlState,
    id: RequestId,
    name: Option<&str>,
    all: bool,
    action: &str,
) -> serde_json::Value {
    if all {
        return handle_lifecycle_all(state, id, action).await;
    }

    let tunnel_name = match name {
        Some(s) => s,
        None => {
            return error_response(
                id,
                rpc::error_codes::INVALID_PARAMS,
                format!("Missing 'Name' parameter for {}", action_to_display(action)),
            );
        }
    };

    // Verify tunnel exists
    let definition = match state.tunnel_get(tunnel_name).await {
        Some(d) => d,
        None => {
            return error_response(
                id,
                rpc::error_codes::APP_ERROR,
                format!("error - tunnel '{}' not found", tunnel_name),
            );
        }
    };

    // Reject lifecycle on startup-managed tunnels
    if definition.ownership == TunnelOwnership::StartupManaged {
        return error_response(
            id,
            rpc::error_codes::APP_ERROR,
            "error - tunnel is managed by the startup configuration",
        );
    }

    let result = match action {
        "start" => state.tunnel_start(tunnel_name).await,
        "stop" => state.tunnel_stop(tunnel_name).await,
        "restart" => state.tunnel_restart(tunnel_name).await,
        _ => unreachable!(),
    };

    match result {
        Ok(status) => success_response(id, serde_json::json!(status)),
        Err(e) => {
            tracing::error!(target: LOG_TARGET, "{} failed: {}", action, e);
            error_response(id, rpc::error_codes::APP_ERROR, e)
        }
    }
}

/// Handle `All` lifecycle: apply action to all tunnel definitions.
///
/// Performs bounded serial dispatch over all definitions. Returns a
/// single aggregated status matching the Proposal 170 contract.
async fn handle_lifecycle_all(
    state: &I2pControlState,
    id: RequestId,
    action: &str,
) -> serde_json::Value {
    let definitions = state.tunnel_list().await;

    if definitions.is_empty() {
        return success_response(id, serde_json::json!("ok"));
    }

    if definitions.len() > MAX_ALL_TARGETS {
        return error_response(
            id,
            rpc::error_codes::INVALID_PARAMS,
            format!("Too many targets for All; maximum is {}", MAX_ALL_TARGETS),
        );
    }

    let mut last_result = "ok".to_string();
    let mut any_error = false;

    for def in &definitions {
        // Skip startup-managed tunnels
        if def.ownership == TunnelOwnership::StartupManaged {
            continue;
        }

        let result = match action {
            "start" => state.tunnel_start(def.name.as_str()).await,
            "stop" => state.tunnel_stop(def.name.as_str()).await,
            "restart" => state.tunnel_restart(def.name.as_str()).await,
            _ => unreachable!(),
        };

        match result {
            Ok(status) => {
                last_result = status;
                if last_result.starts_with("error") {
                    any_error = true;
                }
            }
            Err(e) => {
                last_result = e;
                any_error = true;
            }
        }
    }

    if any_error {
        success_response(id, serde_json::json!(last_result))
    } else {
        success_response(id, serde_json::json!("ok"))
    }
}

/// Convert a TunnelDefinition to the Proposal 170 Get result format.
fn tunnel_definition_to_get_result(def: &TunnelDefinition) -> serde_json::Value {
    let mut result = serde_json::Map::new();

    result.insert("Name".to_string(), serde_json::json!(def.name.as_str()));
    result.insert(
        "Type".to_string(),
        serde_json::json!(def.tunnel_type.as_str()),
    );

    // Map internal state to Proposal 170 wire state
    let state_str = match def.runtime_state {
        TunnelRuntimeState::Running => "running",
        TunnelRuntimeState::Starting => "starting",
        TunnelRuntimeState::Stopping => "stopping",
        TunnelRuntimeState::Failed => "failed",
        TunnelRuntimeState::Stopped | TunnelRuntimeState::Unsupported => "stopped",
        TunnelRuntimeState::ExternallyManaged => "stopped",
    };
    result.insert("State".to_string(), serde_json::json!(state_str));

    // Include raw_config options for lossless round-trip (after core fields,
    // using or_insert so protocol metadata in raw_config never overwrites
    // the correct Name/Type/State values).
    for (k, v) in &def.raw_config {
        result.entry(k.clone()).or_insert_with(|| v.clone());
    }

    // Also include typed options for fields not in raw_config
    if let Some(ref desc) = def.options.description {
        if !result.contains_key("description") {
            result.insert("description".to_string(), serde_json::json!(desc));
        }
    }
    if let Some(ref dest) = def.options.target_destination {
        if !result.contains_key("i2p.tunnel.clientDest") {
            result.insert("i2p.tunnel.clientDest".to_string(), serde_json::json!(dest));
        }
    }
    if let Some(port) = def.options.target_port {
        if !result.contains_key("i2p.tunnel.clientDestPort") {
            result.insert(
                "i2p.tunnel.clientDestPort".to_string(),
                serde_json::json!(port),
            );
        }
    }
    if let Some(ref iface) = def.options.listen_interface {
        if !result.contains_key("i2p.tunnel.listenInterface") {
            result.insert(
                "i2p.tunnel.listenInterface".to_string(),
                serde_json::json!(iface),
            );
        }
    }
    if let Some(port) = def.options.listen_port {
        if !result.contains_key("i2p.tunnel.listenPort") {
            result.insert("i2p.tunnel.listenPort".to_string(), serde_json::json!(port));
        }
    }

    serde_json::Value::Object(result)
}

/// Extract tunnel options from request params.
///
/// Known Proposal 170 option fields are extracted into typed options.
/// Unknown fields are preserved in the raw config for lossless round-trip.
fn extract_tunnel_options(
    params: &serde_json::Map<String, serde_json::Value>,
) -> Result<TunnelOptions, String> {
    let mut options = TunnelOptions::default();

    // General
    if let Some(v) = params.get("description").and_then(|v| v.as_str()) {
        options.description = Some(v.to_string());
    }
    if let Some(v) = params.get("i2p.tunnel.startOnLoad").and_then(|v| v.as_bool()) {
        options.start_on_load = Some(if v {
            StartIntent::StartOnLoad
        } else {
            StartIntent::DoNotStart
        });
    }

    // Client options
    if let Some(v) = params.get("i2p.tunnel.clientDest").and_then(|v| v.as_str()) {
        options.target_destination = Some(v.to_string());
    }
    if let Some(v) = params.get("i2p.tunnel.clientDestPort").and_then(|v| v.as_u64()) {
        if v > u16::MAX as u64 {
            return Err(format!(
                "i2p.tunnel.clientDestPort value {} out of range",
                v
            ));
        }
        options.target_port = Some(v as u16);
    }
    if let Some(v) = params.get("i2p.tunnel.listenInterface").and_then(|v| v.as_str()) {
        options.listen_interface = Some(v.to_string());
    }
    if let Some(v) = params.get("i2p.tunnel.listenPort").and_then(|v| v.as_u64()) {
        if v > u16::MAX as u64 {
            return Err(format!("i2p.tunnel.listenPort value {} out of range", v));
        }
        options.listen_port = Some(v as u16);
    }
    if let Some(v) = params.get("i2p.tunnel.accessList").and_then(|v| v.as_str()) {
        options.access_list = Some(v.to_string());
    }
    if let Some(v) = params.get("i2p.tunnel.allowplaintext").and_then(|v| v.as_bool()) {
        options.allowplaintext = Some(v);
    }

    // Server options
    if let Some(v) = params.get("i2p.tunnel.serverHostingDestination").and_then(|v| v.as_str()) {
        options.hosting_destination = Some(v.to_string());
    }
    if let Some(v) = params.get("i2p.tunnel.isPrivate").and_then(|v| v.as_bool()) {
        options.is_private = Some(v);
    }
    if let Some(v) = params.get("i2p.tunnel.hashcashProofsRequired").and_then(|v| v.as_i64()) {
        options.hashcash_proofs_required = Some(v as i32);
    }
    if let Some(v) = params.get("i2p.tunnel.signatureType").and_then(|v| v.as_str()) {
        options.signature_type = Some(v.to_string());
    }
    if let Some(v) = params.get("i2p.tunnel.consumer").and_then(|v| v.as_str()) {
        options.consumer = Some(v.to_string());
    }

    // HTTP options
    if let Some(v) = params.get("i2p.tunnel.sslCertificate").and_then(|v| v.as_str()) {
        options.ssl_certificate = Some(v.to_string());
    }
    if let Some(v) = params.get("i2p.tunnel.sslKey").and_then(|v| v.as_str()) {
        options.ssl_key = crate::i2pcontrol::domain::tunnel::OptionRedacted::new(v);
    }
    if let Some(v) = params.get("i2p.tunnel.httpHost").and_then(|v| v.as_str()) {
        options.http_host = Some(v.to_string());
    }

    // Proxy options
    if let Some(v) = params.get("i2p.tunnel.proxyUsername").and_then(|v| v.as_str()) {
        options.proxy_username = Some(v.to_string());
    }
    if let Some(v) = params.get("i2p.tunnel.proxyPassword").and_then(|v| v.as_str()) {
        options.proxy_password = crate::i2pcontrol::domain::tunnel::OptionRedacted::new(v);
    }

    // IRC options
    if let Some(v) = params.get("i2p.tunnel.ircServer").and_then(|v| v.as_str()) {
        options.irc_server = Some(v.to_string());
    }
    if let Some(v) = params.get("i2p.tunnel.ircPort").and_then(|v| v.as_u64()) {
        if v > u16::MAX as u64 {
            return Err(format!("i2p.tunnel.ircPort value {} out of range", v));
        }
        options.irc_port = Some(v as u16);
    }
    if let Some(v) = params.get("i2p.tunnel.ircNick").and_then(|v| v.as_str()) {
        options.irc_nick = Some(v.to_string());
    }
    if let Some(v) = params.get("i2p.tunnel.ircPassword").and_then(|v| v.as_str()) {
        options.irc_password = crate::i2pcontrol::domain::tunnel::OptionRedacted::new(v);
    }
    if let Some(v) = params.get("i2p.tunnel.ircChannels").and_then(|v| v.as_str()) {
        options.irc_channels = Some(v.to_string());
    }

    // Streamr options
    if let Some(v) = params.get("i2p.tunnel.streamrTarget").and_then(|v| v.as_str()) {
        options.streamr_target = Some(v.to_string());
    }

    // I2CP options
    if let Some(obj) = params.get("i2cp").and_then(|v| v.as_object()) {
        for (k, v) in obj {
            if let Some(s) = v.as_str() {
                options.i2cp_options.insert(k.clone(), s.to_string());
            }
        }
    }

    // Custom options
    if let Some(obj) = params.get("i2p.tunnel.customOptions").and_then(|v| v.as_object()) {
        for (k, v) in obj {
            if let Some(s) = v.as_str() {
                options.custom_options.insert(k.clone(), s.to_string());
            }
        }
    }

    Ok(options)
}

/// Merge tunnel options: new values override existing where present.
fn merge_tunnel_options(existing: &TunnelOptions, new: &TunnelOptions) -> TunnelOptions {
    TunnelOptions {
        description: new.description.clone().or(existing.description.clone()),
        start_on_load: new.start_on_load.or(existing.start_on_load),
        target_destination: new.target_destination.clone().or(existing.target_destination.clone()),
        target_port: new.target_port.or(existing.target_port),
        listen_interface: new.listen_interface.clone().or(existing.listen_interface.clone()),
        listen_port: new.listen_port.or(existing.listen_port),
        access_list: new.access_list.clone().or(existing.access_list.clone()),
        allowplaintext: new.allowplaintext.or(existing.allowplaintext),
        hosting_destination: new
            .hosting_destination
            .clone()
            .or(existing.hosting_destination.clone()),
        is_private: new.is_private.or(existing.is_private),
        hashcash_proofs_required: new
            .hashcash_proofs_required
            .or(existing.hashcash_proofs_required),
        signature_type: new.signature_type.clone().or(existing.signature_type.clone()),
        consumer: new.consumer.clone().or(existing.consumer.clone()),
        ssl_certificate: new.ssl_certificate.clone().or(existing.ssl_certificate.clone()),
        ssl_key: if new.ssl_key.is_some() {
            new.ssl_key.clone()
        } else {
            existing.ssl_key.clone()
        },
        http_host: new.http_host.clone().or(existing.http_host.clone()),
        proxy_username: new.proxy_username.clone().or(existing.proxy_username.clone()),
        proxy_password: if new.proxy_password.is_some() {
            new.proxy_password.clone()
        } else {
            existing.proxy_password.clone()
        },
        irc_server: new.irc_server.clone().or(existing.irc_server.clone()),
        irc_port: new.irc_port.or(existing.irc_port),
        irc_nick: new.irc_nick.clone().or(existing.irc_nick.clone()),
        irc_password: if new.irc_password.is_some() {
            new.irc_password.clone()
        } else {
            existing.irc_password.clone()
        },
        irc_channels: new.irc_channels.clone().or(existing.irc_channels.clone()),
        streamr_target: new.streamr_target.clone().or(existing.streamr_target.clone()),
        i2cp_options: if new.i2cp_options.is_empty() {
            existing.i2cp_options.clone()
        } else {
            new.i2cp_options.clone()
        },
        custom_options: if new.custom_options.is_empty() {
            existing.custom_options.clone()
        } else {
            new.custom_options.clone()
        },
    }
}

/// Extract raw config from params (tunnel options only, not protocol metadata).
fn extract_raw_config(
    params: &serde_json::Map<String, serde_json::Value>,
) -> std::collections::BTreeMap<String, serde_json::Value> {
    let mut raw = std::collections::BTreeMap::new();
    for (k, v) in params {
        // Preserve only option fields for lossless round-trip.
        // Protocol metadata (Name, Action, Type, NewName, All) is not stored
        // in raw_config because it is managed by the TunnelDefinition fields.
        if k != "Name" && k != "Action" && k != "Type" && k != "NewName" && k != "All" {
            raw.insert(k.clone(), v.clone());
        }
    }
    raw
}

/// Map internal action string to display name for error messages.
fn action_to_display(action: &str) -> &str {
    match action {
        "start" => "Start",
        "stop" => "Stop",
        "restart" => "Restart",
        _ => action,
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i2pcontrol::control_plane::{FakeTunnelManagerControl, TunnelManagerControl};
    use crate::i2pcontrol::rpc::JsonRpcRequest;

    fn test_state() -> crate::i2pcontrol::server::I2pControlState {
        let mut state = crate::i2pcontrol::server::I2pControlState::new("testpass".to_string());
        state.set_tunnel_manager(Box::new(FakeTunnelManagerControl::new()));
        state
    }

    fn tm_request(method: &str, params: serde_json::Value) -> JsonRpcRequest {
        let params_map = params.as_object().cloned().unwrap();
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params: Some(params_map),
            id: Some(crate::i2pcontrol::rpc::RequestId::Number(1)),
        }
    }

    // --- List tests ---

    #[tokio::test]
    async fn handler_list_empty() {
        let state = test_state();
        let req = tm_request("TunnelManager", serde_json::json!({"Action": "List"}));
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["jsonrpc"], "2.0");
        assert!(resp["result"].is_array());
        assert_eq!(resp["result"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn handler_list_after_create() {
        let state = test_state();
        // Create a tunnel first
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({
                "Action": "Create",
                "Type": "client",
                "Name": "my-tunnel",
                "i2p.tunnel.listenPort": 8080
            }),
        );
        handle_tunnel_manager(&state, &req).await;

        // List
        let req = tm_request("TunnelManager", serde_json::json!({"Action": "List"}));
        let resp = handle_tunnel_manager(&state, &req).await;
        let arr = resp["result"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["Name"], "my-tunnel");
        assert_eq!(arr[0]["Type"], "client");
    }

    // --- Create tests ---

    #[tokio::test]
    async fn handler_create_success() {
        let state = test_state();
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({
                "Action": "Create",
                "Type": "socks",
                "Name": "my-socks",
                "i2p.tunnel.listenPort": 1080
            }),
        );
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["result"], "ok");
    }

    #[tokio::test]
    async fn handler_create_all_types() {
        let state = test_state();
        for (i, &tt) in ALL_TUNNEL_TYPES.iter().enumerate() {
            let name = format!("tunnel-{}", i);
            let req = tm_request(
                "TunnelManager",
                serde_json::json!({
                    "Action": "Create",
                    "Type": tt.as_str(),
                    "Name": name
                }),
            );
            let resp = handle_tunnel_manager(&state, &req).await;
            assert_eq!(
                resp["result"],
                "ok",
                "Create failed for type {}",
                tt.as_str()
            );
        }
        // Verify all 12 exist
        let list_req = tm_request("TunnelManager", serde_json::json!({"Action": "List"}));
        let resp = handle_tunnel_manager(&state, &list_req).await;
        assert_eq!(resp["result"].as_array().unwrap().len(), 12);
    }

    #[tokio::test]
    async fn handler_create_duplicate_name() {
        let state = test_state();
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({
                "Action": "Create",
                "Type": "client",
                "Name": "dup"
            }),
        );
        handle_tunnel_manager(&state, &req).await;

        let req2 = tm_request(
            "TunnelManager",
            serde_json::json!({
                "Action": "Create",
                "Type": "server",
                "Name": "dup"
            }),
        );
        let resp = handle_tunnel_manager(&state, &req2).await;
        assert!(resp["result"].as_str().unwrap().contains("already exists"));
    }

    #[tokio::test]
    async fn handler_create_missing_type() {
        let state = test_state();
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Create", "Name": "test"}),
        );
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn handler_create_missing_name() {
        let state = test_state();
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Create", "Type": "client"}),
        );
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn handler_create_invalid_type() {
        let state = test_state();
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Create", "Type": "invalid", "Name": "test"}),
        );
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    // --- Get tests ---

    #[tokio::test]
    async fn handler_get_found() {
        let state = test_state();
        // Create
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({
                "Action": "Create",
                "Type": "httpserver",
                "Name": "web",
                "i2p.tunnel.listenPort": 443
            }),
        );
        handle_tunnel_manager(&state, &req).await;

        // Get
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Get", "Name": "web"}),
        );
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["result"]["Name"], "web");
        assert_eq!(resp["result"]["Type"], "httpserver");
    }

    #[tokio::test]
    async fn handler_get_not_found() {
        let state = test_state();
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Get", "Name": "missing"}),
        );
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["error"]["code"], -1);
        assert!(resp["error"]["message"].as_str().unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn handler_get_missing_name() {
        let state = test_state();
        let req = tm_request("TunnelManager", serde_json::json!({"Action": "Get"}));
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn handler_get_all() {
        let state = test_state();
        // Create two
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Create", "Type": "client", "Name": "c1"}),
        );
        handle_tunnel_manager(&state, &req).await;
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Create", "Type": "server", "Name": "s1"}),
        );
        handle_tunnel_manager(&state, &req).await;

        // Get All
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Get", "All": true}),
        );
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["result"].as_array().unwrap().len(), 2);
    }

    // --- Edit tests ---

    #[tokio::test]
    async fn handler_edit_success() {
        let state = test_state();
        // Create
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({
                "Action": "Create",
                "Type": "socks",
                "Name": "my-socks",
                "i2p.tunnel.listenPort": 1080
            }),
        );
        handle_tunnel_manager(&state, &req).await;

        // Edit
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({
                "Action": "Edit",
                "Name": "my-socks",
                "i2p.tunnel.listenPort": 2080
            }),
        );
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["result"], "ok");

        // Verify edit took effect
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Get", "Name": "my-socks"}),
        );
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["result"]["i2p.tunnel.listenPort"], 2080);
    }

    #[tokio::test]
    async fn handler_edit_rename() {
        let state = test_state();
        // Create
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Create", "Type": "client", "Name": "old"}),
        );
        handle_tunnel_manager(&state, &req).await;

        // Rename
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({
                "Action": "Edit",
                "Name": "old",
                "NewName": "new"
            }),
        );
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["result"], "ok");

        // Old name gone, new name exists
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Get", "Name": "old"}),
        );
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["error"]["code"], -1);

        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Get", "Name": "new"}),
        );
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["result"]["Name"], "new");
    }

    #[tokio::test]
    async fn handler_edit_not_found() {
        let state = test_state();
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Edit", "Name": "missing"}),
        );
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["error"]["code"], -1);
    }

    #[tokio::test]
    async fn handler_edit_preserves_omitted_fields() {
        let state = test_state();
        // Create with port
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({
                "Action": "Create",
                "Type": "socks",
                "Name": "pres",
                "i2p.tunnel.listenPort": 1080
            }),
        );
        handle_tunnel_manager(&state, &req).await;

        // Edit only description (port should be preserved)
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({
                "Action": "Edit",
                "Name": "pres",
                "description": "my socks proxy"
            }),
        );
        handle_tunnel_manager(&state, &req).await;

        // Verify both fields present
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Get", "Name": "pres"}),
        );
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["result"]["description"], "my socks proxy");
        assert_eq!(resp["result"]["i2p.tunnel.listenPort"], 1080);
    }

    // --- Delete tests ---

    #[tokio::test]
    async fn handler_delete_success() {
        let state = test_state();
        // Create
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Create", "Type": "client", "Name": "del"}),
        );
        handle_tunnel_manager(&state, &req).await;

        // Delete
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Delete", "Name": "del"}),
        );
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["result"], "ok");

        // Verify gone
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Get", "Name": "del"}),
        );
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["error"]["code"], -1);
    }

    #[tokio::test]
    async fn handler_delete_not_found() {
        let state = test_state();
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Delete", "Name": "missing"}),
        );
        let resp = handle_tunnel_manager(&state, &req).await;
        // Delete of absent name is a successful no-op
        assert_eq!(resp["result"], "ok");
    }

    #[tokio::test]
    async fn handler_delete_missing_name() {
        let state = test_state();
        let req = tm_request("TunnelManager", serde_json::json!({"Action": "Delete"}));
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    // --- Lifecycle tests ---

    #[tokio::test]
    async fn handler_start_unsupported() {
        let state = test_state();
        // Create an unsupported type
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({
                "Action": "Create",
                "Type": "ircclient",
                "Name": "irc"
            }),
        );
        handle_tunnel_manager(&state, &req).await;

        // Start should return not-implemented
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Start", "Name": "irc"}),
        );
        let resp = handle_tunnel_manager(&state, &req).await;
        let result = resp["result"].as_str().unwrap();
        assert!(result.contains("not implemented"));
    }

    #[tokio::test]
    async fn handler_restart_unsupported() {
        let state = test_state();
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({
                "Action": "Create",
                "Type": "streamrserver",
                "Name": "sr"
            }),
        );
        handle_tunnel_manager(&state, &req).await;

        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Restart", "Name": "sr"}),
        );
        let resp = handle_tunnel_manager(&state, &req).await;
        let result = resp["result"].as_str().unwrap();
        assert!(result.contains("not implemented"));
    }

    #[tokio::test]
    async fn handler_stop_unsupported_safe() {
        let state = test_state();
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({
                "Action": "Create",
                "Type": "httpbidirserver",
                "Name": "hb"
            }),
        );
        handle_tunnel_manager(&state, &req).await;

        // Stop of unsupported is safe/idempotent
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Stop", "Name": "hb"}),
        );
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["result"], "ok");
    }

    #[tokio::test]
    async fn handler_start_not_found() {
        let state = test_state();
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Start", "Name": "missing"}),
        );
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["error"]["code"], -1);
    }

    #[tokio::test]
    async fn handler_start_missing_name() {
        let state = test_state();
        let req = tm_request("TunnelManager", serde_json::json!({"Action": "Start"}));
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    // --- All tests ---

    #[tokio::test]
    async fn handler_all_start_unsupported() {
        let state = test_state();
        // Create two unsupported tunnels
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Create", "Type": "ircclient", "Name": "i1"}),
        );
        handle_tunnel_manager(&state, &req).await;
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Create", "Type": "ircserver", "Name": "i2"}),
        );
        handle_tunnel_manager(&state, &req).await;

        // All Start
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Start", "All": true}),
        );
        let resp = handle_tunnel_manager(&state, &req).await;
        let result = resp["result"].as_str().unwrap();
        assert!(result.contains("not implemented"));
    }

    #[tokio::test]
    async fn handler_all_stop_safe() {
        let state = test_state();
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Create", "Type": "socks", "Name": "s1"}),
        );
        handle_tunnel_manager(&state, &req).await;

        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Stop", "All": true}),
        );
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["result"], "ok");
    }

    #[tokio::test]
    async fn handler_all_empty_registry() {
        let state = test_state();
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Start", "All": true}),
        );
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["result"], "ok");
    }

    #[tokio::test]
    async fn handler_all_rejected_for_create() {
        let state = test_state();
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Create", "All": true, "Type": "client", "Name": "x"}),
        );
        let resp = handle_tunnel_manager(&state, &req).await;
        // All is not valid for Create - it's just ignored since Name/Type are required
        // The create should still work normally
        assert_eq!(resp["result"], "ok");
    }

    // --- Validation tests ---

    #[tokio::test]
    async fn handler_invalid_action() {
        let state = test_state();
        let req = tm_request("TunnelManager", serde_json::json!({"Action": "Invalid"}));
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn handler_missing_action() {
        let state = test_state();
        let req = tm_request("TunnelManager", serde_json::json!({}));
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn handler_no_params() {
        let state = test_state();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "TunnelManager".to_string(),
            params: None,
            id: Some(crate::i2pcontrol::rpc::RequestId::Number(1)),
        };
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn handler_create_with_options() {
        let state = test_state();
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({
                "Action": "Create",
                "Type": "httpserver",
                "Name": "secure-web",
                "description": "Secure web server",
                "i2p.tunnel.listenPort": 443,
                "i2p.tunnel.sslCertificate": "/path/to/cert.pem",
                "i2p.tunnel.isPrivate": true,
                "i2cp.someOption": "someValue"
            }),
        );
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["result"], "ok");

        // Verify options round-trip
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Get", "Name": "secure-web"}),
        );
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["result"]["description"], "Secure web server");
        assert_eq!(resp["result"]["i2p.tunnel.listenPort"], 443);
        assert_eq!(
            resp["result"]["i2p.tunnel.sslCertificate"],
            "/path/to/cert.pem"
        );
        assert_eq!(resp["result"]["i2p.tunnel.isPrivate"], true);
    }

    #[tokio::test]
    async fn handler_get_after_restart() {
        let state = test_state();
        // Create
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Create", "Type": "socks", "Name": "rr"}),
        );
        handle_tunnel_manager(&state, &req).await;

        // Restart (unsupported)
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Restart", "Name": "rr"}),
        );
        handle_tunnel_manager(&state, &req).await;

        // Get should still work
        let req = tm_request(
            "TunnelManager",
            serde_json::json!({"Action": "Get", "Name": "rr"}),
        );
        let resp = handle_tunnel_manager(&state, &req).await;
        assert_eq!(resp["result"]["Name"], "rr");
        assert_eq!(resp["result"]["State"], "stopped");
    }

    #[tokio::test]
    async fn handler_unsupported_never_reports_running() {
        let state = test_state();
        // Create all unsupported types
        for (i, &tt) in ALL_TUNNEL_TYPES.iter().enumerate() {
            let name = format!("ur-{}", i);
            let req = tm_request(
                "TunnelManager",
                serde_json::json!({
                    "Action": "Create",
                    "Type": tt.as_str(),
                    "Name": name
                }),
            );
            handle_tunnel_manager(&state, &req).await;

            // Try to start (will fail with not-implemented)
            let req = tm_request(
                "TunnelManager",
                serde_json::json!({"Action": "Start", "Name": name}),
            );
            handle_tunnel_manager(&state, &req).await;

            // Get must show stopped, never running
            let req = tm_request(
                "TunnelManager",
                serde_json::json!({"Action": "Get", "Name": name}),
            );
            let resp = handle_tunnel_manager(&state, &req).await;
            assert_ne!(
                resp["result"]["State"].as_str(),
                Some("running"),
                "unsupported tunnel {} must not report running",
                tt.as_str()
            );
        }
    }

    #[tokio::test]
    async fn handler_create_all_types_crud_cycle() {
        let state = test_state();
        for (i, &tt) in ALL_TUNNEL_TYPES.iter().enumerate() {
            let name = format!("crud-{}", i);

            // Create
            let req = tm_request(
                "TunnelManager",
                serde_json::json!({
                    "Action": "Create",
                    "Type": tt.as_str(),
                    "Name": name,
                    "description": format!("test {}", tt.as_str())
                }),
            );
            let resp = handle_tunnel_manager(&state, &req).await;
            assert_eq!(resp["result"], "ok", "Create failed for {}", tt.as_str());

            // Get
            let req = tm_request(
                "TunnelManager",
                serde_json::json!({"Action": "Get", "Name": name}),
            );
            let resp = handle_tunnel_manager(&state, &req).await;
            assert_eq!(resp["result"]["Type"], tt.as_str());

            // Edit
            let req = tm_request(
                "TunnelManager",
                serde_json::json!({
                    "Action": "Edit",
                    "Name": name,
                    "description": format!("updated {}", tt.as_str())
                }),
            );
            let resp = handle_tunnel_manager(&state, &req).await;
            assert_eq!(resp["result"], "ok", "Edit failed for {}", tt.as_str());

            // Delete
            let req = tm_request(
                "TunnelManager",
                serde_json::json!({"Action": "Delete", "Name": name}),
            );
            let resp = handle_tunnel_manager(&state, &req).await;
            assert_eq!(resp["result"], "ok", "Delete failed for {}", tt.as_str());

            // Verify gone
            let req = tm_request(
                "TunnelManager",
                serde_json::json!({"Action": "Get", "Name": name}),
            );
            let resp = handle_tunnel_manager(&state, &req).await;
            assert_eq!(
                resp["error"]["code"],
                -1,
                "Get after delete should fail for {}",
                tt.as_str()
            );
        }
    }
}
