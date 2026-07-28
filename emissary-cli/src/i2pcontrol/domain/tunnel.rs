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

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Exact Proposal 170 tunnel type strings.
///
/// Each variant maps to exactly one external wire spelling. No aliases or
/// case-insensitive parsing is introduced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TunnelType {
    #[serde(rename = "client")]
    Client,
    #[serde(rename = "httpclient")]
    HttpClient,
    #[serde(rename = "ircclient")]
    IrcClient,
    #[serde(rename = "socks")]
    Socks,
    #[serde(rename = "socksirc")]
    SocksIrc,
    #[serde(rename = "connectclient")]
    ConnectClient,
    #[serde(rename = "streamrclient")]
    StreamrClient,
    #[serde(rename = "server")]
    Server,
    #[serde(rename = "httpserver")]
    HttpServer,
    #[serde(rename = "httpbidirserver")]
    HttpBidirServer,
    #[serde(rename = "ircserver")]
    IrcServer,
    #[serde(rename = "streamrserver")]
    StreamrServer,
}

/// All valid tunnel types in canonical order.
pub const ALL_TUNNEL_TYPES: &[TunnelType] = &[
    TunnelType::Client,
    TunnelType::HttpClient,
    TunnelType::IrcClient,
    TunnelType::Socks,
    TunnelType::SocksIrc,
    TunnelType::ConnectClient,
    TunnelType::StreamrClient,
    TunnelType::Server,
    TunnelType::HttpServer,
    TunnelType::HttpBidirServer,
    TunnelType::IrcServer,
    TunnelType::StreamrServer,
];

impl TunnelType {
    /// Return the exact external wire string for this tunnel type.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Client => "client",
            Self::HttpClient => "httpclient",
            Self::IrcClient => "ircclient",
            Self::Socks => "socks",
            Self::SocksIrc => "socksirc",
            Self::ConnectClient => "connectclient",
            Self::StreamrClient => "streamrclient",
            Self::Server => "server",
            Self::HttpServer => "httpserver",
            Self::HttpBidirServer => "httpbidirserver",
            Self::IrcServer => "ircserver",
            Self::StreamrServer => "streamrserver",
        }
    }

    /// Parse from an exact external wire string.
    pub fn from_str_exact(s: &str) -> Option<Self> {
        match s {
            "client" => Some(Self::Client),
            "httpclient" => Some(Self::HttpClient),
            "ircclient" => Some(Self::IrcClient),
            "socks" => Some(Self::Socks),
            "socksirc" => Some(Self::SocksIrc),
            "connectclient" => Some(Self::ConnectClient),
            "streamrclient" => Some(Self::StreamrClient),
            "server" => Some(Self::Server),
            "httpserver" => Some(Self::HttpServer),
            "httpbidirserver" => Some(Self::HttpBidirServer),
            "ircserver" => Some(Self::IrcServer),
            "streamrserver" => Some(Self::StreamrServer),
            _ => None,
        }
    }

    /// Returns true if this is a client-type tunnel.
    pub fn is_client(&self) -> bool {
        matches!(
            self,
            Self::Client
                | Self::HttpClient
                | Self::IrcClient
                | Self::Socks
                | Self::SocksIrc
                | Self::ConnectClient
                | Self::StreamrClient
        )
    }

    /// Returns true if this is a server-type tunnel.
    pub fn is_server(&self) -> bool {
        matches!(
            self,
            Self::Server
                | Self::HttpServer
                | Self::HttpBidirServer
                | Self::IrcServer
                | Self::StreamrServer
        )
    }
}

impl fmt::Display for TunnelType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for TunnelType {
    type Err = TunnelTypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_str_exact(s).ok_or_else(|| TunnelTypeError(s.to_string()))
    }
}

/// Error returned when a string is not a valid tunnel type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelTypeError(pub String);

impl fmt::Display for TunnelTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid tunnel type {:?}; expected one of: {}",
            self.0,
            ALL_TUNNEL_TYPES.iter().map(|t| t.as_str()).collect::<Vec<_>>().join(", ")
        )
    }
}

impl std::error::Error for TunnelTypeError {}

/// Exact Proposal 170 TunnelManager action strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum TunnelAction {
    #[serde(rename = "List")]
    List,
    #[serde(rename = "Create")]
    Create,
    #[serde(rename = "Edit")]
    Edit,
    #[serde(rename = "Get")]
    Get,
    #[serde(rename = "Delete")]
    Delete,
    #[serde(rename = "Start")]
    Start,
    #[serde(rename = "Stop")]
    Stop,
    #[serde(rename = "Restart")]
    Restart,
}

/// All valid tunnel actions in canonical order.
pub const ALL_TUNNEL_ACTIONS: &[TunnelAction] = &[
    TunnelAction::List,
    TunnelAction::Create,
    TunnelAction::Edit,
    TunnelAction::Get,
    TunnelAction::Delete,
    TunnelAction::Start,
    TunnelAction::Stop,
    TunnelAction::Restart,
];

impl TunnelAction {
    /// Return the exact external wire string for this action.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::List => "List",
            Self::Create => "Create",
            Self::Edit => "Edit",
            Self::Get => "Get",
            Self::Delete => "Delete",
            Self::Start => "Start",
            Self::Stop => "Stop",
            Self::Restart => "Restart",
        }
    }

    /// Parse from an exact external wire string.
    pub fn from_str_exact(s: &str) -> Option<Self> {
        match s {
            "List" => Some(Self::List),
            "Create" => Some(Self::Create),
            "Edit" => Some(Self::Edit),
            "Get" => Some(Self::Get),
            "Delete" => Some(Self::Delete),
            "Start" => Some(Self::Start),
            "Stop" => Some(Self::Stop),
            "Restart" => Some(Self::Restart),
            _ => None,
        }
    }
}

impl fmt::Display for TunnelAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for TunnelAction {
    type Err = TunnelActionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_str_exact(s).ok_or_else(|| TunnelActionError(s.to_string()))
    }
}

/// Error returned when a string is not a valid tunnel action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelActionError(pub String);

impl fmt::Display for TunnelActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid tunnel action {:?}; expected one of: {}",
            self.0,
            ALL_TUNNEL_ACTIONS.iter().map(|a| a.as_str()).collect::<Vec<_>>().join(", ")
        )
    }
}

impl std::error::Error for TunnelActionError {}

/// A validated, non-empty tunnel name.
///
/// Preserves exact user spelling including case. Whitespace-only names are
/// rejected.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TunnelName(String);

impl TunnelName {
    /// Create a new validated tunnel name.
    pub fn new(name: impl Into<String>) -> Result<Self, TunnelNameError> {
        let s = name.into();
        if s.is_empty() {
            return Err(TunnelNameError::Empty);
        }
        if s.trim().is_empty() {
            return Err(TunnelNameError::WhitespaceOnly);
        }
        Ok(Self(s))
    }

    /// Return the name as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume and return the inner string.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for TunnelName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for TunnelName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Error returned when a tunnel name is invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelNameError {
    Empty,
    WhitespaceOnly,
}

impl fmt::Display for TunnelNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "tunnel name must not be empty"),
            Self::WhitespaceOnly => write!(f, "tunnel name must not be whitespace-only"),
        }
    }
}

impl std::error::Error for TunnelNameError {}

/// Whether a tunnel definition is managed by the Proposal 170 control plane,
/// by an existing startup path, or is an unsupported type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TunnelOwnership {
    /// Managed by the Proposal 170 control plane.
    ControlPlane,
    /// Managed by an existing startup configuration path.
    StartupManaged,
    /// Not supported by any real backend.
    Unsupported,
}

impl fmt::Display for TunnelOwnership {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ControlPlane => write!(f, "control_plane"),
            Self::StartupManaged => write!(f, "startup_managed"),
            Self::Unsupported => write!(f, "unsupported"),
        }
    }
}

/// Internal-only runtime state for a tunnel definition.
///
/// This state is never exposed through the Proposal 170 wire protocol. It
/// tracks whether a tunnel has been started, is starting, running, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TunnelRuntimeState {
    /// The tunnel is not running.
    Stopped,
    /// The tunnel is in the process of starting.
    Starting,
    /// The tunnel is running.
    Running,
    /// The tunnel is in the process of stopping.
    Stopping,
    /// The tunnel has entered a failed state.
    Failed,
    /// The tunnel type has no supported backend.
    Unsupported,
    /// The tunnel is managed outside the control plane (startup-managed).
    ExternallyManaged,
}

impl fmt::Display for TunnelRuntimeState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stopped => write!(f, "stopped"),
            Self::Starting => write!(f, "starting"),
            Self::Running => write!(f, "running"),
            Self::Stopping => write!(f, "stopping"),
            Self::Failed => write!(f, "failed"),
            Self::Unsupported => write!(f, "unsupported"),
            Self::ExternallyManaged => write!(f, "externally_managed"),
        }
    }
}

/// Description of a tunnel's start intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StartIntent {
    /// The tunnel should start when the router starts.
    StartOnLoad,
    /// The tunnel should not start automatically.
    #[default]
    DoNotStart,
}

impl fmt::Display for StartIntent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StartOnLoad => write!(f, "start_on_load"),
            Self::DoNotStart => write!(f, "do_not_start"),
        }
    }
}

/// Complete known Proposal 170 tunnel option model.
///
/// Known fields receive typed storage. Unknown top-level API keys are NOT
/// retained as an extension mechanism. Protocol-defined extensibility
/// containers such as `CustomOptions` are retained exactly.
///
/// Secret fields (passwords, keys) implement redacted Debug/Display.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TunnelOptions {
    // === General ===
    /// Tunnel description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Whether the tunnel should start on router load.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_on_load: Option<StartIntent>,

    // === Client tunnel options ===
    /// Target destination (base64 or base32).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_destination: Option<String>,

    /// Target destination port.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_port: Option<u16>,

    /// Local listen address for proxy tunnels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listen_interface: Option<String>,

    /// Local listen port for proxy tunnels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listen_port: Option<u16>,

    /// Access list for client tunnels (comma-separated destinations).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_list: Option<String>,

    /// Whether to allow plaintext to I2P.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowplaintext: Option<bool>,

    // === Server tunnel options ===
    /// Hosting destination (base64 RouterInfo for server tunnels).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hosting_destination: Option<String>,

    /// Whether the server tunnel is private (hidden service).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_private: Option<bool>,

    /// Hash cash proof level.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hashcash_proofs_required: Option<i32>,

    /// Signature type for the tunnel.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature_type: Option<String>,

    /// Consumer (for server tunnels).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consumer: Option<String>,

    // === HTTP-specific ===
    /// SSL certificate path for HTTPS tunnels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssl_certificate: Option<String>,

    /// SSL key path for HTTPS tunnels.
    #[serde(skip_serializing_if = "OptionRedacted::is_none")]
    pub ssl_key: OptionRedacted,

    /// HTTP host for HTTP tunnels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_host: Option<String>,

    // === Proxy-specific ===
    /// Proxy username for SOCKS/HTTP proxy tunnels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_username: Option<String>,

    /// Proxy password for SOCKS/HTTP proxy tunnels.
    #[serde(skip_serializing_if = "OptionRedacted::is_none")]
    pub proxy_password: OptionRedacted,

    // === IRC-specific ===
    /// IRC server address for IRC tunnels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub irc_server: Option<String>,

    /// IRC server port for IRC tunnels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub irc_port: Option<u16>,

    /// IRC nick for IRC tunnels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub irc_nick: Option<String>,

    /// IRC password for IRC tunnels.
    #[serde(skip_serializing_if = "OptionRedacted::is_none")]
    pub irc_password: OptionRedacted,

    /// IRC channels for IRC tunnels (comma-separated).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub irc_channels: Option<String>,

    // === Streamr-specific ===
    /// Streamr target for streamr tunnels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub streamr_target: Option<String>,

    // === Generic I2CP options ===
    /// I2CP options as a deterministic key-value map.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub i2cp_options: BTreeMap<String, String>,

    /// Custom options (protocol-defined extensibility container).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub custom_options: BTreeMap<String, String>,
}

impl Default for TunnelOptions {
    fn default() -> Self {
        Self {
            description: None,
            start_on_load: None,
            target_destination: None,
            target_port: None,
            listen_interface: None,
            listen_port: None,
            access_list: None,
            allowplaintext: None,
            hosting_destination: None,
            is_private: None,
            hashcash_proofs_required: None,
            signature_type: None,
            consumer: None,
            ssl_certificate: None,
            ssl_key: OptionRedacted::none(),
            http_host: None,
            proxy_username: None,
            proxy_password: OptionRedacted::none(),
            irc_server: None,
            irc_port: None,
            irc_nick: None,
            irc_password: OptionRedacted::none(),
            irc_channels: None,
            streamr_target: None,
            i2cp_options: BTreeMap::new(),
            custom_options: BTreeMap::new(),
        }
    }
}

/// A redacted string wrapper for sensitive values.
///
/// The inner value is stored but redacted in Debug and Display output.
#[derive(Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OptionRedacted(Option<String>);

impl OptionRedacted {
    pub fn new(value: impl Into<String>) -> Self {
        Self(Some(value.into()))
    }

    pub fn none() -> Self {
        Self(None)
    }

    pub fn as_deref(&self) -> Option<&str> {
        self.0.as_deref()
    }

    pub fn is_none(&self) -> bool {
        self.0.is_none()
    }

    pub fn is_some(&self) -> bool {
        self.0.is_some()
    }
}

impl fmt::Debug for OptionRedacted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Some(_) => write!(f, "OptionRedacted(***)"),
            None => write!(f, "OptionRedacted(None)"),
        }
    }
}

impl fmt::Display for OptionRedacted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Some(_) => write!(f, "***"),
            None => write!(f, ""),
        }
    }
}

/// A complete tunnel definition stored by the Proposal 170 control plane.
///
/// This is the canonical round-trip representation consumed by TunnelManager
/// `get` and persisted by the generation store.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TunnelDefinition {
    /// The validated tunnel name (exact user spelling).
    pub name: TunnelName,

    /// The exact Proposal 170 tunnel type.
    pub tunnel_type: TunnelType,

    /// Ownership classification.
    pub ownership: TunnelOwnership,

    /// Internal runtime state (never exposed on the wire).
    pub runtime_state: TunnelRuntimeState,

    /// The start intent for this tunnel.
    pub start_intent: StartIntent,

    /// Complete typed tunnel options.
    pub options: TunnelOptions,

    /// The original raw configuration as received from the API, preserved
    /// for lossless `get` behavior. Keys use exact external field names.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub raw_config: BTreeMap<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tunnel_type_parse_all_variants() {
        for tt in ALL_TUNNEL_TYPES {
            let s = tt.as_str();
            let parsed = TunnelType::from_str_exact(s).expect(s);
            assert_eq!(&parsed, tt);
            assert_eq!(parsed.as_str(), s);
        }
    }

    #[test]
    fn tunnel_type_reject_unknown() {
        assert!(TunnelType::from_str_exact("unknown").is_none());
        assert!(TunnelType::from_str_exact("Client").is_none());
        assert!(TunnelType::from_str_exact("CLIENT").is_none());
        assert!(TunnelType::from_str_exact("").is_none());
    }

    #[test]
    fn tunnel_type_display_matches_wire() {
        for tt in ALL_TUNNEL_TYPES {
            assert_eq!(tt.to_string(), tt.as_str());
        }
    }

    #[test]
    fn tunnel_type_from_str_roundtrip() {
        for tt in ALL_TUNNEL_TYPES {
            let s = tt.as_str();
            let parsed: TunnelType = s.parse().unwrap();
            assert_eq!(parsed, *tt);
        }
    }

    #[test]
    fn tunnel_type_is_client() {
        assert!(TunnelType::Client.is_client());
        assert!(TunnelType::HttpClient.is_client());
        assert!(TunnelType::IrcClient.is_client());
        assert!(TunnelType::Socks.is_client());
        assert!(TunnelType::SocksIrc.is_client());
        assert!(TunnelType::ConnectClient.is_client());
        assert!(TunnelType::StreamrClient.is_client());
        assert!(!TunnelType::Server.is_client());
        assert!(!TunnelType::HttpServer.is_client());
    }

    #[test]
    fn tunnel_type_is_server() {
        assert!(TunnelType::Server.is_server());
        assert!(TunnelType::HttpServer.is_server());
        assert!(TunnelType::HttpBidirServer.is_server());
        assert!(TunnelType::IrcServer.is_server());
        assert!(TunnelType::StreamrServer.is_server());
        assert!(!TunnelType::Client.is_server());
        assert!(!TunnelType::Socks.is_server());
    }

    #[test]
    fn tunnel_type_serialization_exact() {
        let json = serde_json::to_string(&TunnelType::HttpClient).unwrap();
        assert_eq!(json, "\"httpclient\"");
    }

    #[test]
    fn tunnel_type_count() {
        assert_eq!(ALL_TUNNEL_TYPES.len(), 12);
    }

    #[test]
    fn tunnel_action_parse_all_variants() {
        for ta in ALL_TUNNEL_ACTIONS {
            let s = ta.as_str();
            let parsed = TunnelAction::from_str_exact(s).expect(s);
            assert_eq!(&parsed, ta);
            assert_eq!(parsed.as_str(), s);
        }
    }

    #[test]
    fn tunnel_action_reject_unknown() {
        assert!(TunnelAction::from_str_exact("unknown").is_none());
        assert!(TunnelAction::from_str_exact("create").is_none());
        assert!(TunnelAction::from_str_exact("CREATE").is_none());
        assert!(TunnelAction::from_str_exact("").is_none());
    }

    #[test]
    fn tunnel_action_count() {
        assert_eq!(ALL_TUNNEL_ACTIONS.len(), 8);
    }

    #[test]
    fn tunnel_action_serialization_exact() {
        let json = serde_json::to_string(&TunnelAction::Create).unwrap();
        assert_eq!(json, "\"Create\"");
    }

    #[test]
    fn tunnel_name_valid() {
        let name = TunnelName::new("my-tunnel").unwrap();
        assert_eq!(name.as_str(), "my-tunnel");
    }

    #[test]
    fn tunnel_name_empty_rejected() {
        assert_eq!(TunnelName::new(""), Err(TunnelNameError::Empty));
    }

    #[test]
    fn tunnel_name_whitespace_rejected() {
        assert_eq!(TunnelName::new("   "), Err(TunnelNameError::WhitespaceOnly));
    }

    #[test]
    fn tunnel_name_preserves_case() {
        let name = TunnelName::new("MyTunnel").unwrap();
        assert_eq!(name.as_str(), "MyTunnel");
    }

    #[test]
    fn tunnel_name_serialization_roundtrip() {
        let name = TunnelName::new("test-tunnel").unwrap();
        let json = serde_json::to_string(&name).unwrap();
        let deserialized: TunnelName = serde_json::from_str(&json).unwrap();
        assert_eq!(name, deserialized);
    }

    #[test]
    fn option_redacted_debug_redacts() {
        let redacted = OptionRedacted::new("secret123");
        let debug = format!("{:?}", redacted);
        assert_eq!(debug, "OptionRedacted(***)");
        assert!(!debug.contains("secret123"));
    }

    #[test]
    fn option_redacted_display_redacts() {
        let redacted = OptionRedacted::new("secret123");
        let display = format!("{}", redacted);
        assert_eq!(display, "***");
        assert!(!display.contains("secret123"));
    }

    #[test]
    fn option_redacted_none_debug() {
        let redacted = OptionRedacted::none();
        let debug = format!("{:?}", redacted);
        assert_eq!(debug, "OptionRedacted(None)");
    }

    #[test]
    fn option_redacted_none_display() {
        let redacted = OptionRedacted::none();
        let display = format!("{}", redacted);
        assert_eq!(display, "");
    }

    #[test]
    fn tunnel_options_deterministic_serialization() {
        let opts = TunnelOptions {
            description: Some("test".to_string()),
            listen_port: Some(8080),
            ..Default::default()
        };
        let json1 = serde_json::to_string(&opts).unwrap();
        let json2 = serde_json::to_string(&opts).unwrap();
        assert_eq!(json1, json2);
    }

    #[test]
    fn tunnel_options_default_is_empty() {
        let opts = TunnelOptions::default();
        let json = serde_json::to_string(&opts).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn tunnel_definition_serialization_roundtrip() {
        let def = TunnelDefinition {
            name: TunnelName::new("test").unwrap(),
            tunnel_type: TunnelType::Socks,
            ownership: TunnelOwnership::ControlPlane,
            runtime_state: TunnelRuntimeState::Stopped,
            start_intent: StartIntent::DoNotStart,
            options: TunnelOptions {
                listen_port: Some(1080),
                ..Default::default()
            },
            raw_config: BTreeMap::new(),
        };
        let json = serde_json::to_string(&def).unwrap();
        let deserialized: TunnelDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(def, deserialized);
    }

    #[test]
    fn tunnel_definition_deterministic_ordering() {
        let def = TunnelDefinition {
            name: TunnelName::new("test").unwrap(),
            tunnel_type: TunnelType::HttpServer,
            ownership: TunnelOwnership::ControlPlane,
            runtime_state: TunnelRuntimeState::Stopped,
            start_intent: StartIntent::StartOnLoad,
            options: TunnelOptions {
                description: Some("desc".to_string()),
                listen_port: Some(443),
                ssl_key: OptionRedacted::new("key"),
                ..Default::default()
            },
            raw_config: BTreeMap::new(),
        };
        let json = serde_json::to_string_pretty(&def).unwrap();
        // Verify deterministic field ordering (debug, http_host, listen_port, etc.)
        let desc_pos = json.find("description").unwrap();
        let port_pos = json.find("listen_port").unwrap();
        assert!(desc_pos < port_pos);
    }
}
