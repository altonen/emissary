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

use crate::{
    config::{AddressBookConfig, EmissaryConfig},
    ui::dioxus::{
        bandwidth_monitor::{BandwidthMonitor, TimeRange},
        config::{
            AdvancedConfig, ExploratoryConfig, HttpProxyConfig, I2cpConfig, Ntcp2Config,
            PortForwardingConfig, SamConfig, SocksProxyConfig, Ssu2Config, TransitConfig,
        },
        util::{load_addresses, read_b32_address},
    },
};

use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Instant};

/// Selected view in the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarSelection {
    Dashboard,
    Bandwidth,
    HiddenServices,
    AddressBook,
    Settings,
}

/// Router status.
pub enum RouterStatus {
    Active,
    ShuttingDown,
}

/// Router state.
#[derive(Clone, Copy)]
pub struct RouterState {
    /// Number of connected routers.
    pub num_routers: usize,

    /// Number of transit tunnels.
    pub num_transit_tunnels: usize,

    /// Number of tunnel build failures.
    pub num_tunnel_build_failures: usize,

    /// Number of tunnels built.
    pub num_tunnels_built: usize,

    /// Router ID
    pub router_id: &'static str,

    /// Should router ID be displayed.
    pub show_router_id: bool,

    /// Router uptime.
    pub uptime: Instant,
}

impl RouterState {
    /// Create new `RouterState`.
    pub fn new(router_id: &'static str) -> Self {
        Self {
            num_routers: 0usize,
            num_transit_tunnels: 0usize,
            num_tunnel_build_failures: 0usize,
            num_tunnels_built: 0usize,
            router_id,
            show_router_id: false,
            uptime: Instant::now(),
        }
    }
}

/// Traffic options for visualization.
pub struct TrafficOptions {
    /// Show inbound traffic.
    pub show_inbound: bool,

    /// Show outbound traffic.
    pub show_outbound: bool,

    /// Selected time range.
    pub range: TimeRange,
}

/// Router traffic data.
pub struct Traffic {
    /// Inbound bandwidth.
    pub inbound_bandwidth: usize,

    /// Traffic visualization options.
    pub options: TrafficOptions,

    /// Outbound bandwidth.
    pub outbound_bandwidth: usize,

    /// Peak traffict.
    pub peak_traffic: usize,

    /// Previous inbound bandwidth.
    pub prev_inbound_bandwidth: usize,

    /// Previous outbound bandwidth.
    pub prev_outbound_bandwidth: usize,

    /// Total bandwidth monitor.
    pub total_bandwidth: BandwidthMonitor,

    /// Transit bandwidth monitor.
    pub transit_bandwidth: BandwidthMonitor,

    /// Transit inbound bandwidth.
    pub transit_inbound_bandwidth: usize,

    /// Transit inbound bandwidth.
    pub transit_outbound_bandwidth: usize,
}

impl Traffic {
    /// Create new `Traffic`
    pub fn new() -> Self {
        Self {
            outbound_bandwidth: 0usize,
            inbound_bandwidth: 0usize,
            prev_inbound_bandwidth: 0usize,
            prev_outbound_bandwidth: 0usize,
            peak_traffic: 0usize,
            transit_inbound_bandwidth: 0usize,
            transit_outbound_bandwidth: 0usize,
            total_bandwidth: BandwidthMonitor::new(),
            transit_bandwidth: BandwidthMonitor::new(),
            options: TrafficOptions {
                show_inbound: true,
                show_outbound: true,
                range: TimeRange::default(),
            },
        }
    }
}

/// Settings tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Transports,
    Client,
    Proxies,
    Tunnels,
    Advanced,
}

/// Settings status.
#[derive(Debug, Clone)]
pub enum SettingsStatus {
    Idle,
    Saved,
    Error(String),
}

/// Settings info.
pub struct Settings {
    /// Does the settings view contain unsaved changes.
    pub dirty: bool,

    /// Active tab.
    pub active_tab: SettingsTab,

    /// Settings status.
    pub status: SettingsStatus,

    /// NTCP2 config.
    pub ntcp2: Ntcp2Config,

    /// SSU2 settings.
    pub ssu2: Ssu2Config,

    /// Port forwarding settings.
    pub port_forwarding: PortForwardingConfig,

    /// I2CP.
    pub i2cp: I2cpConfig,

    /// SAMv3.
    pub sam: SamConfig,

    /// HTTP proxy.
    pub http_proxy: HttpProxyConfig,

    /// SOCKS proxy.
    pub socks_proxy: SocksProxyConfig,

    /// Transit.
    pub transit: TransitConfig,

    /// Exploratory.
    pub exploratory: ExploratoryConfig,

    /// Advanced.
    pub advanced: AdvancedConfig,
}

impl Settings {
    /// Create new `Settings`.
    pub fn new(config: &EmissaryConfig) -> Self {
        Self {
            active_tab: SettingsTab::Transports,
            advanced: AdvancedConfig::from(config),
            dirty: false,
            exploratory: ExploratoryConfig::from(config),
            http_proxy: HttpProxyConfig::from(config),
            i2cp: I2cpConfig::from(config),
            ntcp2: Ntcp2Config::from(config),
            port_forwarding: PortForwardingConfig::from(config),
            sam: SamConfig::from(config),
            socks_proxy: SocksProxyConfig::from(config),
            ssu2: Ssu2Config::from(config),
            status: SettingsStatus::Idle,
            transit: TransitConfig::from(config),
        }
    }
}

/// Address book tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressBookTab {
    Browse,
    AddDestination,
    Configure,
}

/// Status for "Add destination" view.
#[derive(Debug, Clone)]
pub enum AddDestinationStatus {
    Idle,
    Saved,
    Error(String),
}

/// Fields for "Add destination" tab.
pub struct AddDestination {
    /// Destination.
    pub destination: String,

    /// Hostname.
    pub hostname: String,

    /// Status.
    pub status: AddDestinationStatus,
}

/// Fields for "Browse" tab.
pub struct Browse {
    /// Search term.
    pub search_term: String,

    /// Addresses.
    pub addresses: BTreeMap<Arc<str>, Arc<str>>,

    /// Pending deletion.
    pub pending_delete_server: Option<String>,
}

/// Subscription stauts.
#[derive(Debug, Clone)]
pub enum SubscriptionStatus {
    Idle,
    Saved,
    Error(String),
}

/// Fields for "Subscriptions" tab.
pub struct Subscriptions {
    /// Subscriptions.
    pub subscriptions: String,

    /// Status.
    pub status: SubscriptionStatus,
}

/// Address book-related fields.
pub struct AddressBook {
    /// Active tab.
    pub tab: AddressBookTab,

    /// "Add destination" context.
    pub add_destination: AddDestination,

    /// "Browse" context.
    pub browse: Browse,

    /// "Subscription" context.
    pub subscriptions: Subscriptions,
}

impl AddressBook {
    /// Create new `AddressBook`.
    pub fn new(base_path: PathBuf, config: &EmissaryConfig) -> Self {
        Self {
            tab: AddressBookTab::Browse,
            add_destination: AddDestination {
                destination: String::from(""),
                hostname: String::from(""),
                status: AddDestinationStatus::Idle,
            },
            browse: Browse {
                search_term: String::from(""),
                addresses: load_addresses(base_path.join("addressbook/addresses")),
                pending_delete_server: None,
            },
            subscriptions: Subscriptions {
                subscriptions: match config.address_book {
                    Some(AddressBookConfig {
                        subscriptions: Some(ref subscriptions),
                        ..
                    }) => subscriptions.join(","),
                    _ => String::from(""),
                },
                status: SubscriptionStatus::Idle,
            },
        }
    }
}

/// Server tunnel.
#[derive(Debug, Clone)]
pub struct ServerTunnel {
    /// Server address.
    pub address: String,

    /// Server port.
    pub port: String,

    /// Path to key file.
    pub path: String,
}

/// Client tunnel.
#[derive(Debug, Clone)]
pub struct ClientTunnel {
    /// Local bind address.
    pub address: String,

    /// Local bind port.
    pub port: String,

    /// Destination address.
    pub destination: String,

    /// Destination port.
    pub destination_port: String,
}

/// Active "Hidden services" status.
#[derive(Debug, Clone)]
pub enum HiddenServicesStatus {
    CreateServer(Option<String>),
    EditServer(Option<String>),
    CreateClient(Option<String>),
    EditClient(Option<String>),
}

/// Server edit information.
pub struct ServerEdit {
    /// New name.
    pub name: String,

    /// Original name.
    pub original_name: String,

    /// Local bind port.
    pub port: String,

    /// Path to key file.
    pub path: String,
}

/// Server tunnel information.
pub struct ServerInfo {
    /// Server tunnels.
    pub servers: BTreeMap<String, ServerTunnel>,

    /// Server edit fields.
    pub edit: ServerEdit,

    /// Pending deletion.
    pub pending_delete: Option<String>,
}

impl From<&EmissaryConfig> for ServerInfo {
    fn from(value: &EmissaryConfig) -> Self {
        let servers = value.server_tunnels.as_ref().map_or_else(BTreeMap::default, |services| {
            services
                .iter()
                .map(|server| {
                    let address = read_b32_address(&server.destination_path)
                        .map(|address| format!("{address}.b32.i2p"))
                        .unwrap_or(String::from("Key file does not exist"));
                    (
                        server.name.clone(),
                        ServerTunnel {
                            port: server.port.to_string(),
                            path: server.destination_path.clone(),
                            address,
                        },
                    )
                })
                .collect()
        });

        Self {
            servers,
            pending_delete: None,
            edit: ServerEdit {
                name: String::from(""),
                original_name: String::from(""),
                port: String::from(""),
                path: String::from(""),
            },
        }
    }
}

/// Client edit information.
pub struct ClientEdit {
    /// New name.
    pub name: String,

    /// Original name.
    pub original_name: String,

    /// Local bind address.
    pub address: String,

    /// Local bind port.
    pub port: String,

    /// Destination address.
    pub destination: String,

    /// Destination .port
    pub destination_port: String,
}

/// Client tunnel information.
pub struct ClientInfo {
    /// Client tunnels.
    pub clients: BTreeMap<String, ClientTunnel>,

    /// Client edit information.
    pub edit: ClientEdit,

    /// Pending deletion.
    pub pending_delete: Option<String>,
}

impl From<&EmissaryConfig> for ClientInfo {
    fn from(value: &EmissaryConfig) -> Self {
        let clients = value.client_tunnels.as_ref().map_or_else(BTreeMap::default, |tunnels| {
            tunnels
                .iter()
                .map(|client| {
                    (
                        client.name.clone(),
                        ClientTunnel {
                            address: client.address.clone().unwrap_or(String::from("127.0.0.1")),
                            port: client.port.to_string(),
                            destination: client.destination.clone(),
                            destination_port: client
                                .destination_port
                                .map_or_else(|| String::from("80"), |port| port.to_string()),
                        },
                    )
                })
                .collect()
        });

        Self {
            clients,
            pending_delete: None,
            edit: ClientEdit {
                name: String::from(""),
                original_name: String::from(""),
                address: String::from(""),
                port: String::from(""),
                destination: String::from(""),
                destination_port: String::from(""),
            },
        }
    }
}

/// Hidden services-related fields.
pub struct HiddenServices {
    /// Current status.
    pub status: Option<HiddenServicesStatus>,

    /// Server infomation.
    pub server: ServerInfo,

    /// Client information.
    pub client: ClientInfo,
}

impl HiddenServices {
    /// Create new `HiddenServices`.
    pub fn new(config: &EmissaryConfig) -> Self {
        HiddenServices {
            status: None,
            server: ServerInfo::from(config),
            client: ClientInfo::from(config),
        }
    }
}
