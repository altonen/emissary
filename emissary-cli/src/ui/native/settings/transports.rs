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

use std::net::{Ipv4Addr, Ipv6Addr};

use crate::ui::native::{types::Message, RouterUi};

use iced::{
    advanced::widget::Text,
    border::Radius,
    widget::{Checkbox, Column, Container, TextInput},
    Background, Border, Color, Theme,
};

#[derive(Debug, Clone)]
pub struct Ntcp2Config {
    port: Option<String>,
    ipv4_host: Option<String>,
    ipv6_host: Option<String>,
    ipv4: Option<bool>,
    ipv6: Option<bool>,
    publish: Option<bool>,
    enabled: bool,
}

impl Ntcp2Config {
    fn port(&self) -> &str {
        self.port.as_ref().map_or("", |port| port.as_str())
    }

    fn ipv4_host(&self) -> &str {
        self.ipv4_host.as_ref().map_or("", |host| host.as_str())
    }

    fn ipv6_host(&self) -> &str {
        self.ipv6_host.as_ref().map_or("", |host| host.as_str())
    }

    fn published(&self) -> bool {
        self.publish.unwrap_or(false)
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn ipv4_enabled(&self) -> bool {
        self.ipv4.unwrap_or(true)
    }

    pub fn ipv6_enabled(&self) -> bool {
        self.ipv6.unwrap_or(true)
    }

    pub fn set_port(&mut self, port: String) {
        self.port = Some(port);
    }

    pub fn set_ipv4_host(&mut self, host: String) {
        self.ipv4_host = Some(host);
    }

    pub fn set_ipv6_host(&mut self, host: String) {
        self.ipv6_host = Some(host);
    }

    pub fn set_published(&mut self, published: bool) {
        self.publish = Some(published);
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn set_ipv4_enabled(&mut self, enabled: bool) {
        self.ipv4 = Some(enabled);
    }

    pub fn set_ipv6_enabled(&mut self, enabled: bool) {
        self.ipv6 = Some(enabled);
    }
}

impl TryInto<Option<crate::config::Ntcp2Config>> for Ntcp2Config {
    type Error = String;

    fn try_into(self) -> Result<Option<crate::config::Ntcp2Config>, Self::Error> {
        if !self.enabled {
            return Ok(None);
        }

        Ok(Some(crate::config::Ntcp2Config {
            port: match self.port {
                Some(port) =>
                    port.parse::<u16>().map_err(|_| String::from("Invalid NTCP2 port"))?,
                None => 0,
            },
            ipv4_host: match self.ipv4_host.as_ref() {
                None => None,
                Some(host) if host.is_empty() => None,
                Some(host) => Some(
                    host.parse::<Ipv4Addr>()
                        .map_err(|_| String::from("Invalid NTCP2 IPv4 address"))?,
                ),
            },
            ipv6_host: match self.ipv6_host.as_ref() {
                None => None,
                Some(host) if host.is_empty() => None,
                Some(host) => Some(
                    host.parse::<Ipv6Addr>()
                        .map_err(|_| String::from("Invalid NTCP2 IPv6 address"))?,
                ),
            },
            ipv4: self.ipv4,
            ipv6: self.ipv6,
            publish: self.publish,
            ml_kem: Some(4),
            disable_pq: None,
        }))
    }
}

impl From<&Option<crate::config::Ntcp2Config>> for Ntcp2Config {
    fn from(value: &Option<crate::config::Ntcp2Config>) -> Self {
        match value {
            Some(value) => Self {
                port: Some(value.port.to_string()),
                ipv4_host: value.ipv4_host.map(|address| address.to_string()),
                ipv6_host: value.ipv6_host.map(|address| address.to_string()),
                ipv4: value.ipv4,
                ipv6: value.ipv6,
                publish: value.publish,
                enabled: true,
            },
            None => Self {
                port: None,
                ipv4_host: None,
                ipv6_host: None,
                ipv4: None,
                ipv6: None,
                publish: None,
                enabled: false,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct Ssu2Config {
    port: Option<String>,
    ipv4: Option<bool>,
    ipv4_host: Option<String>,
    ipv4_mtu: Option<String>,
    ipv6: Option<bool>,
    ipv6_host: Option<String>,
    ipv6_mtu: Option<String>,
    publish: Option<bool>,
    enabled: bool,
}

impl Ssu2Config {
    fn port(&self) -> &str {
        self.port.as_ref().map_or("", |port| port.as_str())
    }

    fn ipv4_host(&self) -> &str {
        self.ipv4_host.as_ref().map_or("", |host| host.as_str())
    }

    fn ipv4_mtu(&self) -> &str {
        self.ipv4_mtu.as_ref().map_or("", |mtu| mtu.as_str())
    }

    fn ipv6_host(&self) -> &str {
        self.ipv6_host.as_ref().map_or("", |host| host.as_str())
    }

    fn ipv6_mtu(&self) -> &str {
        self.ipv6_mtu.as_ref().map_or("", |mtu| mtu.as_str())
    }

    fn published(&self) -> bool {
        self.publish.unwrap_or(false)
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn ipv4_enabled(&self) -> bool {
        self.ipv4.unwrap_or(true)
    }

    pub fn ipv6_enabled(&self) -> bool {
        self.ipv6.unwrap_or(true)
    }

    pub fn set_port(&mut self, port: String) {
        self.port = Some(port);
    }

    pub fn set_ipv4_host(&mut self, host: String) {
        self.ipv4_host = Some(host);
    }

    pub fn set_ipv6_host(&mut self, host: String) {
        self.ipv6_host = Some(host);
    }

    pub fn set_ipv4_mtu(&mut self, mtu: String) {
        self.ipv4_mtu = Some(mtu);
    }

    pub fn set_ipv6_mtu(&mut self, mtu: String) {
        self.ipv6_mtu = Some(mtu);
    }

    pub fn set_published(&mut self, published: bool) {
        self.publish = Some(published);
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn set_ipv4_enabled(&mut self, enabled: bool) {
        self.ipv4 = Some(enabled);
    }

    pub fn set_ipv6_enabled(&mut self, enabled: bool) {
        self.ipv6 = Some(enabled);
    }
}

impl TryInto<Option<crate::config::Ssu2Config>> for Ssu2Config {
    type Error = String;

    fn try_into(self) -> Result<Option<crate::config::Ssu2Config>, Self::Error> {
        if !self.enabled {
            return Ok(None);
        }

        Ok(Some(crate::config::Ssu2Config {
            port: match self.port {
                Some(port) => port.parse::<u16>().map_err(|_| String::from("Invalid SSU2 port"))?,
                None => 0,
            },
            ipv4_host: self
                .ipv4_host
                .as_ref()
                .map(|s| {
                    s.parse::<Ipv4Addr>().map_err(|_| String::from("Invalid SSU2 IPv4 address"))
                })
                .transpose()?,
            ipv6_host: self
                .ipv6_host
                .as_ref()
                .map(|s| {
                    s.parse::<Ipv6Addr>().map_err(|_| String::from("Invalid SSU2 IPv6 address"))
                })
                .transpose()?,
            publish: self.publish,
            ipv4: self.ipv4,
            ipv6: self.ipv6,
            ipv4_mtu: self
                .ipv4_mtu
                .as_ref()
                .map(|s| s.parse::<usize>().map_err(|_| String::from("Invalid SSU2 IPv4 MTU")))
                .transpose()?,
            ipv6_mtu: self
                .ipv6_mtu
                .as_ref()
                .map(|s| s.parse::<usize>().map_err(|_| String::from("Invalid SSU2 IPv6 MTU")))
                .transpose()?,
        }))
    }
}

impl From<&Option<crate::config::Ssu2Config>> for Ssu2Config {
    fn from(value: &Option<crate::config::Ssu2Config>) -> Self {
        match value {
            Some(value) => Self {
                port: Some(value.port.to_string()),
                ipv4_host: value.ipv4_host.map(|address| address.to_string()),
                ipv6_host: value.ipv6_host.map(|address| address.to_string()),
                ipv4_mtu: value.ipv4_mtu.map(|mtu| mtu.to_string()),
                ipv6_mtu: value.ipv6_mtu.map(|mtu| mtu.to_string()),
                ipv4: value.ipv4,
                ipv6: value.ipv6,
                publish: value.publish,
                enabled: true,
            },
            None => Self {
                port: None,
                ipv4_host: None,
                ipv6_host: None,
                ipv4: None,
                ipv6: None,
                ipv4_mtu: None,
                ipv6_mtu: None,
                publish: None,
                enabled: false,
            },
        }
    }
}

#[derive(Clone)]
pub struct PortForwardingConfig {
    pub nat_pmp: bool,
    pub upnp: bool,
}

impl TryInto<Option<crate::config::PortForwardingConfig>> for PortForwardingConfig {
    type Error = String;

    fn try_into(self) -> Result<Option<crate::config::PortForwardingConfig>, Self::Error> {
        if !self.nat_pmp && !self.upnp {
            return Ok(None);
        }

        Ok(Some(crate::config::PortForwardingConfig {
            nat_pmp: self.nat_pmp,
            upnp: self.upnp,
            name: String::from("emissary"),
        }))
    }
}

impl From<&Option<crate::config::PortForwardingConfig>> for PortForwardingConfig {
    fn from(value: &Option<crate::config::PortForwardingConfig>) -> Self {
        match value {
            Some(value) => Self {
                nat_pmp: value.nat_pmp,
                upnp: value.upnp,
            },
            None => Self {
                nat_pmp: false,
                upnp: false,
            },
        }
    }
}

impl RouterUi {
    pub fn transport_settings<'a>(&self, mut settings: Column<'a, Message>) -> Column<'a, Message> {
        let ntcp2_port = self.ntcp2.port();
        let ntcp2_ipv4_host = self.ntcp2.ipv4_host();
        let ntcp2_ipv6_host = self.ntcp2.ipv6_host();
        let ntcp2_ipv4_enabled = self.ntcp2.ipv4_enabled();
        let ntcp2_ipv6_enabled = self.ntcp2.ipv6_enabled();
        let ntcp2_published = self.ntcp2.published();

        let ntcp2 = Container::new(
            Column::new()
                .push(Text::new("NTCP2"))
                .push(Text::new("Port").size(15).color(Color::from_rgb8(0x9b, 0xa2, 0xae)))
                .push(
                    TextInput::new("Port", ntcp2_port)
                        .size(15)
                        .on_input(Message::Ntcp2PortChanged)
                        .padding(10)
                        .style(
                            |_theme: &Theme, _status: _| iced::widget::text_input::Style {
                                border: Border {
                                    radius: Radius::from(6.0),
                                    width: 1.0,
                                    color: Color::from_rgb8(28, 36, 49),
                                },
                                background: iced::Background::Color(iced::Color::from_rgb8(
                                    0x37, 0x41, 0x51,
                                )),
                                icon: Color::WHITE,
                                placeholder: Color::from_rgb8(0x9b, 0xa2, 0xae),
                                value: Color::from_rgb8(0xf3, 0xf3, 0xf2),
                                selection: Color::from_rgb8(0x9b, 0xa2, 0xae),
                            },
                        ),
                )
                .push(Text::new("IPv4 Host").size(15).color(Color::from_rgb8(0x9b, 0xa2, 0xae)))
                .push(
                    TextInput::new("Host", ntcp2_ipv4_host)
                        .size(15)
                        .on_input(Message::Ntcp2Ipv4HostChanged)
                        .padding(10)
                        .style(
                            |_theme: &Theme, _status: _| iced::widget::text_input::Style {
                                border: Border {
                                    radius: Radius::from(6.0),
                                    width: 1.0,
                                    color: Color::from_rgb8(28, 36, 49),
                                },
                                background: iced::Background::Color(iced::Color::from_rgb8(
                                    0x37, 0x41, 0x51,
                                )),
                                icon: Color::WHITE,
                                placeholder: Color::from_rgb8(0x9b, 0xa2, 0xae),
                                value: Color::from_rgb8(0xf3, 0xf3, 0xf2),
                                selection: Color::from_rgb8(0x9b, 0xa2, 0xae),
                            },
                        ),
                )
                .push(Text::new("IPv6 Host").size(15).color(Color::from_rgb8(0x9b, 0xa2, 0xae)))
                .push(
                    TextInput::new("Host", ntcp2_ipv6_host)
                        .size(15)
                        .on_input(Message::Ntcp2Ipv6HostChanged)
                        .padding(10)
                        .style(
                            |_theme: &Theme, _status: _| iced::widget::text_input::Style {
                                border: Border {
                                    radius: Radius::from(6.0),
                                    width: 1.0,
                                    color: Color::from_rgb8(28, 36, 49),
                                },
                                background: iced::Background::Color(iced::Color::from_rgb8(
                                    0x37, 0x41, 0x51,
                                )),
                                icon: Color::WHITE,
                                placeholder: Color::from_rgb8(0x9b, 0xa2, 0xae),
                                value: Color::from_rgb8(0xf3, 0xf3, 0xf2),
                                selection: Color::from_rgb8(0x9b, 0xa2, 0xae),
                            },
                        ),
                )
                .push(
                    Checkbox::new(ntcp2_published)
                        .label("Publish")
                        .size(15)
                        .on_toggle(Message::Ntcp2Published)
                        .style(|_theme: &Theme, status: _| iced::widget::checkbox::Style {
                            text_color: Some(Color::from_rgb8(0x9b, 0xa2, 0xae)),
                            background: match status {
                                iced::widget::checkbox::Status::Active { is_checked }
                                | iced::widget::checkbox::Status::Hovered { is_checked }
                                    if is_checked =>
                                    Background::Color(Color::from_rgb8(0x60, 0x82, 0xb6)),
                                _ => Background::Color(Color::from_rgb8(28, 36, 49)),
                            },
                            icon_color: Color::WHITE,
                            border: Border {
                                radius: Radius::from(1.0),
                                width: 1.0,
                                color: Color::from_rgb8(0x60, 0x82, 0xb6),
                            },
                        }),
                )
                .push(
                    Checkbox::new(ntcp2_ipv4_enabled)
                        .label("Enable IPv4")
                        .size(15)
                        .on_toggle(Message::Ntcp2Ipv4Enabled)
                        .style(|_theme: &Theme, status: _| iced::widget::checkbox::Style {
                            text_color: Some(Color::from_rgb8(0x9b, 0xa2, 0xae)),
                            background: match status {
                                iced::widget::checkbox::Status::Active { is_checked }
                                | iced::widget::checkbox::Status::Hovered { is_checked }
                                    if is_checked =>
                                    Background::Color(Color::from_rgb8(0x60, 0x82, 0xb6)),
                                _ => Background::Color(Color::from_rgb8(28, 36, 49)),
                            },
                            icon_color: Color::WHITE,
                            border: Border {
                                radius: Radius::from(1.0),
                                width: 1.0,
                                color: Color::from_rgb8(0x60, 0x82, 0xb6),
                            },
                        }),
                )
                .push(
                    Checkbox::new(ntcp2_ipv6_enabled)
                        .label("Enable IPv6")
                        .size(15)
                        .on_toggle(Message::Ntcp2Ipv6Enabled)
                        .style(|_theme: &Theme, status: _| iced::widget::checkbox::Style {
                            text_color: Some(Color::from_rgb8(0x9b, 0xa2, 0xae)),
                            background: match status {
                                iced::widget::checkbox::Status::Active { is_checked }
                                | iced::widget::checkbox::Status::Hovered { is_checked }
                                    if is_checked =>
                                    Background::Color(Color::from_rgb8(0x60, 0x82, 0xb6)),
                                _ => Background::Color(Color::from_rgb8(28, 36, 49)),
                            },
                            icon_color: Color::WHITE,
                            border: Border {
                                radius: Radius::from(1.0),
                                width: 1.0,
                                color: Color::from_rgb8(0x60, 0x82, 0xb6),
                            },
                        }),
                )
                .push(
                    Checkbox::new(self.ntcp2.enabled())
                        .label("Enable")
                        .size(15)
                        .on_toggle(Message::Ntcp2Enabled)
                        .style(|_theme: &Theme, status: _| iced::widget::checkbox::Style {
                            text_color: Some(Color::from_rgb8(0x9b, 0xa2, 0xae)),
                            background: match status {
                                iced::widget::checkbox::Status::Active { is_checked }
                                | iced::widget::checkbox::Status::Hovered { is_checked }
                                    if is_checked =>
                                    Background::Color(Color::from_rgb8(0x60, 0x82, 0xb6)),
                                _ => Background::Color(Color::from_rgb8(28, 36, 49)),
                            },
                            icon_color: Color::WHITE,
                            border: Border {
                                radius: Radius::from(1.0),
                                width: 1.0,
                                color: Color::from_rgb8(0x60, 0x82, 0xb6),
                            },
                        }),
                )
                .spacing(5),
        )
        .padding(10);

        let ssu2_port = self.ssu2.port();
        let ssu2_ipv4_host = self.ssu2.ipv4_host();
        let ssu2_ipv6_host = self.ssu2.ipv6_host();
        let ssu2_ipv4_mtu = self.ssu2.ipv4_mtu();
        let ssu2_ipv6_mtu = self.ssu2.ipv6_mtu();
        let ssu2_ipv4_enabled = self.ssu2.ipv4_enabled();
        let ssu2_ipv6_enabled = self.ssu2.ipv6_enabled();
        let ssu2_published = self.ssu2.published();

        let ssu2 = Container::new(
            Column::new()
                .push(Text::new("SSU2"))
                .push(Text::new("Port").size(15).color(Color::from_rgb8(0x9b, 0xa2, 0xae)))
                .push(
                    TextInput::new("Port", ssu2_port)
                        .size(15)
                        .on_input(Message::Ssu2PortChanged)
                        .padding(10)
                        .style(
                            |_theme: &Theme, _status: _| iced::widget::text_input::Style {
                                border: Border {
                                    radius: Radius::from(6.0),
                                    width: 1.0,
                                    color: Color::from_rgb8(28, 36, 49),
                                },
                                background: iced::Background::Color(iced::Color::from_rgb8(
                                    0x37, 0x41, 0x51,
                                )),
                                icon: Color::WHITE,
                                placeholder: Color::from_rgb8(0x9b, 0xa2, 0xae),
                                value: Color::from_rgb8(0xf3, 0xf3, 0xf2),
                                selection: Color::from_rgb8(0x9b, 0xa2, 0xae),
                            },
                        ),
                )
                .push(Text::new("IPv4 Host").size(15).color(Color::from_rgb8(0x9b, 0xa2, 0xae)))
                .push(
                    TextInput::new("Host", ssu2_ipv4_host)
                        .size(15)
                        .on_input(Message::Ssu2Ipv4HostChanged)
                        .padding(10)
                        .style(
                            |_theme: &Theme, _status: _| iced::widget::text_input::Style {
                                border: Border {
                                    radius: Radius::from(6.0),
                                    width: 1.0,
                                    color: Color::from_rgb8(28, 36, 49),
                                },
                                background: iced::Background::Color(iced::Color::from_rgb8(
                                    0x37, 0x41, 0x51,
                                )),
                                icon: Color::WHITE,
                                placeholder: Color::from_rgb8(0x9b, 0xa2, 0xae),
                                value: Color::from_rgb8(0xf3, 0xf3, 0xf2),
                                selection: Color::from_rgb8(0x9b, 0xa2, 0xae),
                            },
                        ),
                )
                .push(Text::new("IPv4 MTU").size(15).color(Color::from_rgb8(0x9b, 0xa2, 0xae)))
                .push(
                    TextInput::new("MTU", ssu2_ipv4_mtu)
                        .size(15)
                        .on_input(Message::Ssu2Ipv4MtuChanged)
                        .padding(10)
                        .style(
                            |_theme: &Theme, _status: _| iced::widget::text_input::Style {
                                border: Border {
                                    radius: Radius::from(6.0),
                                    width: 1.0,
                                    color: Color::from_rgb8(28, 36, 49),
                                },
                                background: iced::Background::Color(iced::Color::from_rgb8(
                                    0x37, 0x41, 0x51,
                                )),
                                icon: Color::WHITE,
                                placeholder: Color::from_rgb8(0x9b, 0xa2, 0xae),
                                value: Color::from_rgb8(0xf3, 0xf3, 0xf2),
                                selection: Color::from_rgb8(0x9b, 0xa2, 0xae),
                            },
                        ),
                )
                .push(Text::new("IPv6 Host").size(15).color(Color::from_rgb8(0x9b, 0xa2, 0xae)))
                .push(
                    TextInput::new("Host", ssu2_ipv6_host)
                        .size(15)
                        .on_input(Message::Ssu2Ipv6HostChanged)
                        .padding(10)
                        .style(
                            |_theme: &Theme, _status: _| iced::widget::text_input::Style {
                                border: Border {
                                    radius: Radius::from(6.0),
                                    width: 1.0,
                                    color: Color::from_rgb8(28, 36, 49),
                                },
                                background: iced::Background::Color(iced::Color::from_rgb8(
                                    0x37, 0x41, 0x51,
                                )),
                                icon: Color::WHITE,
                                placeholder: Color::from_rgb8(0x9b, 0xa2, 0xae),
                                value: Color::from_rgb8(0xf3, 0xf3, 0xf2),
                                selection: Color::from_rgb8(0x9b, 0xa2, 0xae),
                            },
                        ),
                )
                .push(Text::new("IPv6 MTU").size(15).color(Color::from_rgb8(0x9b, 0xa2, 0xae)))
                .push(
                    TextInput::new("MTU", ssu2_ipv6_mtu)
                        .size(15)
                        .on_input(Message::Ssu2Ipv6MtuChanged)
                        .padding(10)
                        .style(
                            |_theme: &Theme, _status: _| iced::widget::text_input::Style {
                                border: Border {
                                    radius: Radius::from(6.0),
                                    width: 1.0,
                                    color: Color::from_rgb8(28, 36, 49),
                                },
                                background: iced::Background::Color(iced::Color::from_rgb8(
                                    0x37, 0x41, 0x51,
                                )),
                                icon: Color::WHITE,
                                placeholder: Color::from_rgb8(0x9b, 0xa2, 0xae),
                                value: Color::from_rgb8(0xf3, 0xf3, 0xf2),
                                selection: Color::from_rgb8(0x9b, 0xa2, 0xae),
                            },
                        ),
                )
                .push(
                    Checkbox::new(ssu2_published)
                        .label("Publish")
                        .text_size(15)
                        .on_toggle(Message::Ssu2Published)
                        .style(|_theme: &Theme, status: _| iced::widget::checkbox::Style {
                            text_color: Some(Color::from_rgb8(0x9b, 0xa2, 0xae)),
                            background: match status {
                                iced::widget::checkbox::Status::Active { is_checked }
                                | iced::widget::checkbox::Status::Hovered { is_checked }
                                    if is_checked =>
                                    Background::Color(Color::from_rgb8(0x60, 0x82, 0xb6)),
                                _ => Background::Color(Color::from_rgb8(28, 36, 49)),
                            },
                            icon_color: Color::WHITE,
                            border: Border {
                                radius: Radius::from(1.0),
                                width: 1.0,
                                color: Color::from_rgb8(0x60, 0x82, 0xb6),
                            },
                        }),
                )
                .push(
                    Checkbox::new(ssu2_ipv4_enabled)
                        .label("IPv4 enabled")
                        .text_size(15)
                        .on_toggle(Message::Ssu2Ipv4Enabled)
                        .style(|_theme: &Theme, status: _| iced::widget::checkbox::Style {
                            text_color: Some(Color::from_rgb8(0x9b, 0xa2, 0xae)),
                            background: match status {
                                iced::widget::checkbox::Status::Active { is_checked }
                                | iced::widget::checkbox::Status::Hovered { is_checked }
                                    if is_checked =>
                                    Background::Color(Color::from_rgb8(0x60, 0x82, 0xb6)),
                                _ => Background::Color(Color::from_rgb8(28, 36, 49)),
                            },
                            icon_color: Color::WHITE,
                            border: Border {
                                radius: Radius::from(1.0),
                                width: 1.0,
                                color: Color::from_rgb8(0x60, 0x82, 0xb6),
                            },
                        }),
                )
                .push(
                    Checkbox::new(ssu2_ipv6_enabled)
                        .label("IPv6 enabled")
                        .text_size(15)
                        .on_toggle(Message::Ssu2Ipv6Enabled)
                        .style(|_theme: &Theme, status: _| iced::widget::checkbox::Style {
                            text_color: Some(Color::from_rgb8(0x9b, 0xa2, 0xae)),
                            background: match status {
                                iced::widget::checkbox::Status::Active { is_checked }
                                | iced::widget::checkbox::Status::Hovered { is_checked }
                                    if is_checked =>
                                    Background::Color(Color::from_rgb8(0x60, 0x82, 0xb6)),
                                _ => Background::Color(Color::from_rgb8(28, 36, 49)),
                            },
                            icon_color: Color::WHITE,
                            border: Border {
                                radius: Radius::from(1.0),
                                width: 1.0,
                                color: Color::from_rgb8(0x60, 0x82, 0xb6),
                            },
                        }),
                )
                .push(
                    Checkbox::new(self.ssu2.enabled())
                        .label("Enable")
                        .text_size(15)
                        .on_toggle(Message::Ssu2Enabled)
                        .style(|_theme: &Theme, status: _| iced::widget::checkbox::Style {
                            text_color: Some(Color::from_rgb8(0x9b, 0xa2, 0xae)),
                            background: match status {
                                iced::widget::checkbox::Status::Active { is_checked }
                                | iced::widget::checkbox::Status::Hovered { is_checked }
                                    if is_checked =>
                                    Background::Color(Color::from_rgb8(0x60, 0x82, 0xb6)),
                                _ => Background::Color(Color::from_rgb8(28, 36, 49)),
                            },
                            icon_color: Color::WHITE,
                            border: Border {
                                radius: Radius::from(1.0),
                                width: 1.0,
                                color: Color::from_rgb8(0x60, 0x82, 0xb6),
                            },
                        }),
                )
                .spacing(5),
        )
        .padding(10);

        let port_forwarding = Container::new(
            Column::new()
                .push(Text::new("Port forwarding"))
                .push(
                    Checkbox::new(self.port_forwarding.nat_pmp)
                        .label("NAT-PMP")
                        .text_size(15)
                        .on_toggle(Message::NatPmpEnabled)
                        .style(|_theme: &Theme, status: _| iced::widget::checkbox::Style {
                            text_color: Some(Color::from_rgb8(0x9b, 0xa2, 0xae)),
                            background: match status {
                                iced::widget::checkbox::Status::Active { is_checked }
                                | iced::widget::checkbox::Status::Hovered { is_checked }
                                    if is_checked =>
                                    Background::Color(Color::from_rgb8(0x60, 0x82, 0xb6)),
                                _ => Background::Color(Color::from_rgb8(28, 36, 49)),
                            },
                            icon_color: Color::WHITE,
                            border: Border {
                                radius: Radius::from(1.0),
                                width: 1.0,
                                color: Color::from_rgb8(0x60, 0x82, 0xb6),
                            },
                        }),
                )
                .push(
                    Checkbox::new(self.port_forwarding.upnp)
                        .label("UPnP")
                        .text_size(15)
                        .on_toggle(Message::UpnpEnabled)
                        .style(|_theme: &Theme, status: _| iced::widget::checkbox::Style {
                            text_color: Some(Color::from_rgb8(0x9b, 0xa2, 0xae)),
                            background: match status {
                                iced::widget::checkbox::Status::Active { is_checked }
                                | iced::widget::checkbox::Status::Hovered { is_checked }
                                    if is_checked =>
                                    Background::Color(Color::from_rgb8(0x60, 0x82, 0xb6)),
                                _ => Background::Color(Color::from_rgb8(28, 36, 49)),
                            },
                            icon_color: Color::WHITE,
                            border: Border {
                                radius: Radius::from(1.0),
                                width: 1.0,
                                color: Color::from_rgb8(0x60, 0x82, 0xb6),
                            },
                        }),
                )
                .spacing(10),
        )
        .padding(10);

        settings = settings.push(ntcp2);
        settings = settings.push(ssu2);
        settings = settings.push(port_forwarding);
        settings
    }
}
