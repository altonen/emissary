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

use serde::{Deserialize, Serialize};

/// JSON-RPC 2.0 request ID.
///
/// Preserves string or numeric request IDs as required by JSON-RPC 2.0.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum RequestId {
    String(String),
    Number(i64),
    Null,
}

/// JSON-RPC 2.0 request.
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcRequest {
    /// Must be exactly `"2.0"`.
    pub jsonrpc: String,

    /// Method name.
    pub method: String,

    /// Named parameters (object). Positional params are rejected.
    #[serde(default)]
    pub params: Option<serde_json::Map<String, serde_json::Value>>,

    /// Request ID. Null for notifications (no response sent).
    #[serde(default)]
    pub id: Option<RequestId>,
}

/// JSON-RPC 2.0 success response envelope.
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcSuccess {
    pub jsonrpc: &'static str,
    pub id: RequestId,
    pub result: serde_json::Value,
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcErrorObject {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 error response envelope.
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcErrorResponse {
    pub jsonrpc: &'static str,
    pub id: RequestId,
    pub error: JsonRpcErrorObject,
}

/// JSON-RPC 2.0 error codes.
pub mod error_codes {
    /// Parse error: invalid JSON.
    pub const PARSE_ERROR: i32 = -32700;

    /// Invalid request: valid JSON but not a valid JSON-RPC request.
    pub const INVALID_REQUEST: i32 = -32600;

    /// Method not found.
    pub const METHOD_NOT_FOUND: i32 = -32601;

    /// Invalid params.
    pub const INVALID_PARAMS: i32 = -32602;

    /// Internal error.
    pub const INTERNAL_ERROR: i32 = -32603;

    /// Application-defined error (e.g., authentication failure).
    pub const APP_ERROR: i32 = -1;
}

impl JsonRpcSuccess {
    /// Create a success response.
    pub fn new(id: RequestId, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result,
        }
    }
}

impl JsonRpcErrorResponse {
    /// Create an error response.
    pub fn new(id: RequestId, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            error: JsonRpcErrorObject {
                code,
                message: message.into(),
                data: None,
            },
        }
    }

    /// Create an error response with data.
    pub fn with_data(
        id: RequestId,
        code: i32,
        message: impl Into<String>,
        data: serde_json::Value,
    ) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            error: JsonRpcErrorObject {
                code,
                message: message.into(),
                data: Some(data),
            },
        }
    }
}

/// Parse a JSON-RPC request from a raw body string.
///
/// Returns the parsed request or an error response.
pub fn parse_request(body: &str) -> Result<JsonRpcRequest, JsonRpcErrorResponse> {
    let raw: serde_json::Value = serde_json::from_str(body).map_err(|e| {
        JsonRpcErrorResponse::new(
            RequestId::Null,
            error_codes::PARSE_ERROR,
            format!("Parse error: {e}"),
        )
    })?;

    // Must be a JSON object at top level
    let obj = raw.as_object().ok_or_else(|| {
        JsonRpcErrorResponse::new(
            RequestId::Null,
            error_codes::INVALID_REQUEST,
            "Request must be a JSON object",
        )
    })?;

    // Extract jsonrpc version
    let jsonrpc = obj.get("jsonrpc").and_then(|v| v.as_str()).ok_or_else(|| {
        JsonRpcErrorResponse::new(
            RequestId::Null,
            error_codes::INVALID_REQUEST,
            "Missing 'jsonrpc' field",
        )
    })?;

    if jsonrpc != "2.0" {
        return Err(JsonRpcErrorResponse::new(
            RequestId::Null,
            error_codes::INVALID_REQUEST,
            "jsonrpc must be exactly \"2.0\"",
        ));
    }

    // Extract method
    let method = obj.get("method").and_then(|v| v.as_str()).ok_or_else(|| {
        JsonRpcErrorResponse::new(
            RequestId::Null,
            error_codes::INVALID_REQUEST,
            "Missing 'method' field",
        )
    })?;

    if method.is_empty() {
        return Err(JsonRpcErrorResponse::new(
            RequestId::Null,
            error_codes::INVALID_REQUEST,
            "Method must not be empty",
        ));
    }

    // Extract id
    let id = obj.get("id").cloned().map(RequestId::from_json);

    // Extract params — must be an object if present (named params)
    let params = match obj.get("params") {
        Some(serde_json::Value::Object(map)) => Some(map.clone()),
        Some(_) => {
            return Err(JsonRpcErrorResponse::new(
                id.unwrap_or(RequestId::Null),
                error_codes::INVALID_PARAMS,
                "Params must be a JSON object (named parameters)",
            ));
        }
        None => None,
    };

    Ok(JsonRpcRequest {
        jsonrpc: jsonrpc.to_string(),
        method: method.to_string(),
        params,
        id,
    })
}

impl RequestId {
    fn from_json(val: serde_json::Value) -> Self {
        match val {
            serde_json::Value::String(s) => RequestId::String(s),
            serde_json::Value::Number(n) => RequestId::Number(n.as_i64().unwrap_or(0)),
            serde_json::Value::Null => RequestId::Null,
            _ => RequestId::Null,
        }
    }
}

/// I2PControl method names.
pub mod methods {
    /// Authenticate method.
    pub const AUTHENTICATE: &str = "Authenticate";

    /// RouterInfo method.
    pub const ROUTER_INFO: &str = "RouterInfo";

    /// AddressBook method.
    pub const ADDRESS_BOOK: &str = "AddressBook";

    /// TunnelManager method.
    pub const TUNNEL_MANAGER: &str = "TunnelManager";

    /// ClientServicesInfo method.
    pub const CLIENT_SERVICES_INFO: &str = "ClientServicesInfo";

    /// GetKeys method.
    pub const GET_KEYS: &str = "GetKeys";

    /// SetConfig method.
    pub const SET_CONFIG: &str = "SetConfig";

    /// SetSubscriptions method.
    pub const SET_SUBSCRIPTIONS: &str = "SetSubscriptions";
}

/// Proposal 170 tunnel types.
pub mod tunnel_types {
    pub const CLIENT: &str = "client";
    pub const HTTP_CLIENT: &str = "httpclient";
    pub const IRC_CLIENT: &str = "ircclient";
    pub const SOCKS: &str = "socks";
    pub const SOCKS_IRC: &str = "socksirc";
    pub const CONNECT_CLIENT: &str = "connectclient";
    pub const STREAMR_CLIENT: &str = "streamrclient";
    pub const SERVER: &str = "server";
    pub const HTTP_SERVER: &str = "httpserver";
    pub const HTTP_BIDIR_SERVER: &str = "httpbidirserver";
    pub const IRC_SERVER: &str = "ircserver";
    pub const STREAMR_SERVER: &str = "streamrserver";

    /// All valid Proposal 170 tunnel types.
    pub const ALL: &[&str] = &[
        CLIENT,
        HTTP_CLIENT,
        IRC_CLIENT,
        SOCKS,
        SOCKS_IRC,
        CONNECT_CLIENT,
        STREAMR_CLIENT,
        SERVER,
        HTTP_SERVER,
        HTTP_BIDIR_SERVER,
        IRC_SERVER,
        STREAMR_SERVER,
    ];
}

/// TunnelManager actions.
pub mod tunnel_actions {
    pub const LIST: &str = "List";
    pub const CREATE: &str = "Create";
    pub const EDIT: &str = "Edit";
    pub const GET: &str = "Get";
    pub const DELETE: &str = "Delete";
    pub const START: &str = "Start";
    pub const STOP: &str = "Stop";
    pub const RESTART: &str = "Restart";
}

/// AddressBook books.
pub mod address_books {
    pub const PRIVATE: &str = "private";
    pub const LOCAL: &str = "local";
    pub const ROUTER: &str = "router";
    pub const PUBLISHED: &str = "published";

    pub const ALL: &[&str] = &[PRIVATE, LOCAL, ROUTER, PUBLISHED];
}

/// AddressBook request modes.
pub mod address_book_requests {
    pub const LIST: &str = "List";
    pub const LOOKUP: &str = "Lookup";
    pub const ADD: &str = "Add";
    pub const UPDATE: &str = "Update";
    pub const DELETE: &str = "Delete";
}

/// Authenticate request parameters.
#[derive(Debug, Clone, Deserialize)]
pub struct AuthenticateParams {
    /// API version. Must be 1 or 2.
    #[serde(rename = "API")]
    pub api: Option<i32>,

    /// Username. Must be "i2pcontrol".
    #[serde(rename = "Username")]
    pub username: Option<String>,

    /// Password.
    #[serde(rename = "Password")]
    pub password: Option<String>,
}

/// Authenticate result.
#[derive(Debug, Clone, Serialize)]
#[allow(non_snake_case)]
pub struct AuthenticateResult {
    /// Authentication token.
    pub Token: String,

    /// Negotiated API version.
    pub API: String,
}

/// Proposal 170 RouterInfo selector keys.
/// https://i2p.net/en/proposals/170-i2pcontrol-expansion/
pub mod router_info_keys {
    pub const UDP_ACTIVE: &str = "i2p.router.udp.active";
    pub const VERSION: &str = "i2p.router.version";
    pub const UPTIME: &str = "i2p.router.uptime";
    pub const IDENTITY: &str = "i2p.router.identity";
    pub const NETDB_ACTIVE: &str = "i2p.router.netdb.active";
    pub const NETDB_KNOWN_PROFILES: &str = "i2p.router.netdb.knownProfiles";
    pub const NETDB_ACTIVE_PROFILES: &str = "i2p.router.netdb.activeProfiles";
    pub const BW_INBOUND_1S: &str = "i2p.router.bw.inbound.1s";
    pub const BW_OUTBOUND_1S: &str = "i2p.router.bw.outbound.1s";
    pub const TCP_ACTIVE: &str = "i2p.router.tcp.active";

    // Address-book selectors (owned by M003)
    pub const ADDRESS_BOOK_PRIVATE: &str = "i2p.router.addressbook.private";
    pub const ADDRESS_BOOK_LOCAL: &str = "i2p.router.addressbook.local";
    pub const ADDRESS_BOOK_ROUTER: &str = "i2p.router.addressbook.router";
    pub const ADDRESS_BOOK_PUBLISHED: &str = "i2p.router.addressbook.published";
    pub const ADDRESS_BOOK_SUBSCRIPTIONS: &str = "i2p.router.addressbook.subscriptions";
    pub const ADDRESS_BOOK_CONFIG: &str = "i2p.router.addressbook.config";

    /// All address-book selector keys.
    pub const ADDRESS_BOOK_KEYS: &[&str] = &[
        ADDRESS_BOOK_PRIVATE,
        ADDRESS_BOOK_LOCAL,
        ADDRESS_BOOK_ROUTER,
        ADDRESS_BOOK_PUBLISHED,
        ADDRESS_BOOK_SUBSCRIPTIONS,
        ADDRESS_BOOK_CONFIG,
    ];
}

/// Test if a string is a valid tunnel type.
pub fn is_valid_tunnel_type(s: &str) -> bool {
    tunnel_types::ALL.contains(&s)
}

/// Test if a string is a valid address book name.
pub fn is_valid_address_book(s: &str) -> bool {
    address_books::ALL.contains(&s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_request() {
        let body = r#"{"jsonrpc":"2.0","method":"Authenticate","params":{"API":2,"Username":"i2pcontrol","Password":"secret"},"id":1}"#;
        let req = parse_request(body).unwrap();
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.method, "Authenticate");
        assert_eq!(req.id, Some(RequestId::Number(1)));
        assert!(req.params.is_some());
    }

    #[test]
    fn parse_request_missing_jsonrpc() {
        let body = r#"{"method":"Authenticate","id":1}"#;
        let err = parse_request(body).unwrap_err();
        assert_eq!(err.error.code, error_codes::INVALID_REQUEST);
    }

    #[test]
    fn parse_request_wrong_version() {
        let body = r#"{"jsonrpc":"1.0","method":"Authenticate","id":1}"#;
        let err = parse_request(body).unwrap_err();
        assert_eq!(err.error.code, error_codes::INVALID_REQUEST);
    }

    #[test]
    fn parse_request_missing_method() {
        let body = r#"{"jsonrpc":"2.0","id":1}"#;
        let err = parse_request(body).unwrap_err();
        assert_eq!(err.error.code, error_codes::INVALID_REQUEST);
    }

    #[test]
    fn parse_request_invalid_json() {
        let body = "not json";
        let err = parse_request(body).unwrap_err();
        assert_eq!(err.error.code, error_codes::PARSE_ERROR);
    }

    #[test]
    fn parse_request_positional_params() {
        let body = r#"{"jsonrpc":"2.0","method":"Authenticate","params":["a","b"],"id":1}"#;
        let err = parse_request(body).unwrap_err();
        assert_eq!(err.error.code, error_codes::INVALID_PARAMS);
    }

    #[test]
    fn parse_request_notification() {
        let body = r#"{"jsonrpc":"2.0","method":"Authenticate"}"#;
        let req = parse_request(body).unwrap();
        assert!(req.id.is_none());
    }

    #[test]
    fn parse_request_string_id() {
        let body = r#"{"jsonrpc":"2.0","method":"Authenticate","id":"abc"}"#;
        let req = parse_request(body).unwrap();
        assert_eq!(req.id, Some(RequestId::String("abc".to_string())));
    }

    #[test]
    fn tunnel_types_complete() {
        assert_eq!(tunnel_types::ALL.len(), 12);
        for tt in tunnel_types::ALL {
            assert!(is_valid_tunnel_type(tt));
        }
        assert!(!is_valid_tunnel_type("unknown"));
    }

    #[test]
    fn address_books_complete() {
        assert_eq!(address_books::ALL.len(), 4);
        for ab in address_books::ALL {
            assert!(is_valid_address_book(ab));
        }
        assert!(!is_valid_address_book("unknown"));
    }

    #[test]
    fn serialize_success_response() {
        let resp = JsonRpcSuccess::new(
            RequestId::Number(1),
            serde_json::json!({"Token": "abc", "API": "2"}),
        );
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"result\""));
    }

    #[test]
    fn serialize_error_response() {
        let resp = JsonRpcErrorResponse::new(
            RequestId::Number(1),
            error_codes::METHOD_NOT_FOUND,
            "Method not found",
        );
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"error\""));
        assert!(json.contains("-32601"));
    }
}
