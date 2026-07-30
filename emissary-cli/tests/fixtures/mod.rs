use serde_json::{json, Value};

/// Valid Authenticate request fixture.
#[allow(dead_code)]
pub fn valid_authenticate_request(id: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "Authenticate",
        "params": {
            "API": 2,
            "Username": "i2pcontrol",
            "Password": "test-password"
        },
        "id": id
    })
}

/// Missing jsonrpc field.
#[allow(dead_code)]
pub fn missing_jsonrpc() -> Value {
    json!({
        "method": "Authenticate",
        "params": {},
        "id": 1
    })
}

/// Wrong jsonrpc version.
#[allow(dead_code)]
pub fn wrong_jsonrpc_version() -> Value {
    json!({
        "jsonrpc": "1.0",
        "method": "Authenticate",
        "params": {},
        "id": 1
    })
}

/// Missing method field.
#[allow(dead_code)]
pub fn missing_method() -> Value {
    json!({
        "jsonrpc": "2.0",
        "params": {},
        "id": 1
    })
}

/// Empty method.
#[allow(dead_code)]
pub fn empty_method() -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "",
        "params": {},
        "id": 1
    })
}

/// Positional params (array instead of object).
#[allow(dead_code)]
pub fn positional_params() -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "Authenticate",
        "params": ["a", "b"],
        "id": 1
    })
}

/// Unknown method.
#[allow(dead_code)]
pub fn unknown_method() -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "UnknownMethod",
        "params": {},
        "id": 1
    })
}

/// Notification (null id).
#[allow(dead_code)]
pub fn notification() -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "Authenticate",
        "params": {}
    })
}

/// String ID.
#[allow(dead_code)]
pub fn string_id() -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "Authenticate",
        "params": {
            "API": 2,
            "Username": "i2pcontrol",
            "Password": "test-password"
        },
        "id": "abc-123"
    })
}

/// Authenticate with wrong API version.
#[allow(dead_code)]
pub fn wrong_api_version() -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "Authenticate",
        "params": {
            "API": 3,
            "Username": "i2pcontrol",
            "Password": "test-password"
        },
        "id": 1
    })
}

/// Authenticate with wrong username.
#[allow(dead_code)]
pub fn wrong_username() -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "Authenticate",
        "params": {
            "API": 2,
            "Username": "wrong",
            "Password": "test-password"
        },
        "id": 1
    })
}

/// Authenticate with wrong password.
#[allow(dead_code)]
pub fn wrong_password() -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "Authenticate",
        "params": {
            "API": 2,
            "Username": "i2pcontrol",
            "Password": "wrong-password"
        },
        "id": 1
    })
}

/// Authenticate with missing fields.
#[allow(dead_code)]
pub fn missing_auth_fields() -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "Authenticate",
        "params": {
            "API": 2
        },
        "id": 1
    })
}

/// Non-JSON body.
#[allow(dead_code)]
pub fn not_json() -> &'static str {
    "this is not json"
}

/// JSON array instead of object.
#[allow(dead_code)]
pub fn json_array() -> Value {
    json!([{"jsonrpc": "2.0", "method": "Authenticate", "id": 1}])
}

/// Empty params object for Authenticate.
#[allow(dead_code)]
pub fn empty_params() -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "Authenticate",
        "params": {},
        "id": 1
    })
}

/// Oversized body (1MB+).
#[allow(dead_code)]
pub fn oversized_body() -> String {
    let mut body = String::from(r#"{"jsonrpc":"2.0","method":"Authenticate","params":{"#);
    body.push_str(&" ".repeat(1024 * 1024));
    body.push_str(r#"},"id":1}"#);
    body
}

/// All valid tunnel types.
#[allow(dead_code)]
pub fn all_tunnel_types() -> Vec<&'static str> {
    vec![
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
    ]
}

/// All valid address book names.
#[allow(dead_code)]
pub fn all_address_books() -> Vec<&'static str> {
    vec!["private", "local", "router", "published"]
}

/// All valid TunnelManager actions.
#[allow(dead_code)]
pub fn all_tunnel_actions() -> Vec<&'static str> {
    vec![
        "List", "Create", "Edit", "Get", "Delete", "Start", "Stop", "Restart",
    ]
}
