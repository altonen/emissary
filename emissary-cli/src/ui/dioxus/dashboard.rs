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
    config::PortForwardingConfig,
    ui::{
        calculate_bandwidth,
        dioxus::{
            bandwidth::{BandwidthChart, ChartSource},
            svg::*,
            types::RouterState,
            AppState,
        },
    },
};

use dioxus::prelude::*;

/// Create status card.
fn status_card(title: &str, value: String, icon_svg: &str) -> Element {
    rsx! {
        div {
            class: "status-card",
            div {
                class: "status-card-icon",
                span { style: "display:inline-flex;width:28px;height:28px;", dangerous_inner_html: icon_svg }
            }
            div {
                class: "status-card-info",
                div { class: "status-card-title", "{title}" }
                div { class: "status-card-value", "{value}" }
            }
        }
    }
}

#[component]
pub fn Dashboard() -> Element {
    let mut state = use_context::<SyncSignal<AppState>>();

    let RouterState {
        num_routers,
        num_transit_tunnels,
        num_tunnel_build_failures,
        num_tunnels_built,
        router_id,
        show_router_id,
        uptime,
    } = state.read().router_state();
    let inbound_bandwidth = state.read().traffic.lock().expect("to succeed").inbound_bandwidth;
    let outbound_bandwidth = state.read().traffic.lock().expect("to succeed").outbound_bandwidth;

    // create router's network status info
    let (net_label, net_color) = state.read().network_status();

    // calculate router uptime
    let uptime = {
        let mut secs = uptime.elapsed().as_secs();
        if secs == 0 {
            secs = 1;
        }
        format!("{} h {} min {} s", secs / 3600, (secs / 60) % 60, secs % 60)
    };

    // calculate total bandwidth usage
    let (total_val, total_unit) =
        calculate_bandwidth((inbound_bandwidth + outbound_bandwidth) as f64);

    let build_rate = if num_tunnels_built + num_tunnel_build_failures > 0 {
        ((num_tunnels_built as f64 / (num_tunnels_built + num_tunnel_build_failures) as f64)
            * 100.0) as u32
    } else {
        0
    };

    let (http_text, http_class) = match &state.read().config.http_proxy {
        None => (String::from("Disabled"), "panel-value disabled"),
        Some(config) => (format!("Port {}", config.port), "panel-value enabled"),
    };
    let (socks_text, socks_class) = match &state.read().config.socks_proxy {
        None => (String::from("Disabled"), "panel-value disabled"),
        Some(config) => (format!("Port {}", config.port), "panel-value enabled"),
    };
    let (i2cp_text, i2cp_class) = match &state.read().config.i2cp {
        None => (String::from("Disabled"), "panel-value disabled"),
        Some(config) => (format!("Port {}", config.port), "panel-value enabled"),
    };
    let (sam_tcp_text, sam_tcp_class) = match &state.read().config.sam {
        None => (String::from("Disabled"), "panel-value disabled"),
        Some(config) => (format!("Port {}", config.tcp_port), "panel-value enabled"),
    };
    let (sam_udp_text, sam_udp_class) = match &state.read().config.sam {
        None => (String::from("Disabled"), "panel-value disabled"),
        Some(config) => (format!("Port {}", config.udp_port), "panel-value enabled"),
    };
    let (pf_text, pf_class) = match &state.read().config.port_forwarding {
        None => (String::from("Off"), "panel-value disabled"),
        Some(PortForwardingConfig { nat_pmp: true, .. }) =>
            (String::from("NAT-PMP"), "panel-value enabled"),
        Some(PortForwardingConfig { upnp: true, .. }) =>
            (String::from("UPnP"), "panel-value enabled"),
        _ => (String::from("Off"), "panel-value disabled"),
    };
    let ipv4_status = state.read().ipv4_status.clone();
    let ipv6_status = state.read().ipv6_status.clone();

    rsx! {
        div {
            class: "page",

            div {
                class: "page-title",
                h1 { "Dashboard" }
                p { "Monitor your I2P router" }
            }

            // status cards
            div {
                class: "status-cards",

                div {
                    class: "status-card",
                    div {
                        class: "status-card-icon",
                        span { style: "display:inline-flex;width:28px;height:28px;", dangerous_inner_html: NETWORK_STATUS_SVG }
                    }
                    div {
                        class: "status-card-info",
                        div { class: "status-card-title", "Status" }
                        div {
                            class: "status-card-value-row",
                            span { class: "status-dot", style: "background:{net_color};" }
                            span { class: "status-card-value", "{net_label}" }
                        }
                    }
                }
                { status_card("Connected routers", num_routers.to_string(), ROUTERS_SVG) }
                { status_card("Transit tunnels", num_transit_tunnels.to_string(), TUNNELS_SVG) }
                { status_card("Tunnel success rate", format!("{build_rate}%"), TBSR_SVG)} ,
                { status_card("Total transferred", format!("{total_val:.2} {total_unit}"), BANDWIDTH_SVG) }
            }

            // bandwidth graph
            div { class: "bandwidth-card",
                div { class: "bandwidth-card-title", "Bandwidth usage" }
                div { class: "chart-container",
                    BandwidthChart { source: ChartSource::Total }
                }
                div { class: "chart-legend",
                    button {
                        class: "chart-legend-btn",
                        style: "color: #4682b4;",
                        onclick: move |_| {
                            let app = state.write();
                            let mut traffic = app.traffic.lock().unwrap();
                            traffic.options.show_outbound = !traffic.options.show_outbound;
                        },
                        "● Inbound"
                    }
                    button {
                        class: "chart-legend-btn",
                        style: "color: #ffa500;",
                        onclick: move |_| {
                            let app = state.write();
                            let mut traffic = app.traffic.lock().unwrap();
                            traffic.options.show_inbound = !traffic.options.show_inbound;
                        },
                        "● Outbound"
                    }
                }
            }

            // bottom panels
            div {
                class: "bottom-panels",
                div { class: "panel",
                    div {
                        class: "panel-title",
                        "Services"
                    }
                    div {
                        class: "panel-row",
                        span { class: "panel-label", "HTTP Proxy" }
                        span { class: http_class, "{http_text}" }
                    }
                    div {
                        class: "panel-row",
                        span { class: "panel-label", "SOCKS Proxy" }
                        span { class: socks_class, "{socks_text}" }
                    }
                    div {
                        class: "panel-row",
                        span { class: "panel-label", "I2CP" }
                        span { class: i2cp_class, "{i2cp_text}" }
                    }
                    div {
                        class: "panel-row",
                        span { class: "panel-label", "SAMv3 TCP" }
                        span { class: sam_tcp_class, "{sam_tcp_text}" }
                    }
                    div {
                        class: "panel-row",
                        span { class: "panel-label", "SAMv3 UDP" }
                        span { class: sam_udp_class, "{sam_udp_text}" }
                    }
                    div {
                        class: "panel-row",
                        span { class: "panel-label", "Port forwarding" }
                        span { class: pf_class, "{pf_text}" }
                    }
                }
                div {
                    class: "panel",
                    div { class: "panel-title", "Router information" }
                    div {
                        class: "panel-row",
                        span { class: "panel-label", "Router version" }
                        span { class: "panel-value", { format!("v{}", env!("CARGO_PKG_VERSION")) } }
                    }
                    div {
                        class: "panel-row",
                        span { class: "panel-label", "Router ID" }
                        button {
                            class: "router-id-btn",
                            onclick: move |_| {
                                state.write().state.show_router_id = !show_router_id;
                            },
                            if show_router_id {
                                span { style: "font-size:11px;word-break:break-all;", "{router_id}" }
                            } else {
                                span { "Click to reveal" }
                            }
                        }
                    }
                    div {
                        class: "panel-row",
                        span { class: "panel-label", "IPv4 status" }
                        span { class: "panel-value", "{ipv4_status}" }
                    }
                    div {
                        class: "panel-row",
                        span { class: "panel-label", "IPv6 status" }
                        span { class: "panel-value", "{ipv6_status}" }
                    }
                    div {
                        class: "panel-row",
                        span { class: "panel-label", "Uptime" }
                        span { class: "panel-value", "{uptime}" }
                    }
                }
            }
        }
    }
}
