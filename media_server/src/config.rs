use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;

use tokio::sync::mpsc::Sender;

use crate::acceptor::SSLConfig;
use crate::GLOBAL_CONFIG;
use crate::http::SessionCommand;

pub struct Config {
    pub session_command_sender: Sender<SessionCommand>,
    pub ssl_config: SSLConfig,
    pub tcp_server_config: TCPServerConfig,
    pub udp_server_config: UDPServerConfig,
}

const TCP_ADDRESS_ENV: &'static str = "TCP_ADDRESS";
const TCP_PORT_ENV: &'static str = "TCP_PORT";
const UDP_ADDRESS_ENV: &'static str = "UDP_ADDRESS";
const UDP_PORT_ENV: &'static str = "UDP_PORT";
const WHIP_TOKEN_ENV: &'static str = "WHIP_TOKEN";

impl Config {
    pub fn initialize(sender: Sender<SessionCommand>) -> Self {
        let ssl_config = SSLConfig::new();

        let tcp_ip = IpAddr::from_str(
            &std::env::var(TCP_ADDRESS_ENV)
                .expect(&format!("{TCP_ADDRESS_ENV} env variable should be present")),
        )
        .expect(&format!("${TCP_ADDRESS_ENV} should be valid IPAddr"));
        let tcp_port = std::env::var(TCP_PORT_ENV)
            .map(|port| {
                port.parse::<u16>()
                    .expect(&format!("{TCP_PORT_ENV} should be u16 integer"))
            })
            .expect(&format!("{TCP_PORT_ENV} env variable should be present"));

        let tcp_address = SocketAddr::new(tcp_ip, tcp_port);

        let udp_ip = IpAddr::from_str(
            &std::env::var(UDP_ADDRESS_ENV)
                .expect(&format!("{UDP_ADDRESS_ENV} env variable should be present")),
        )
        .expect(&format!("${UDP_ADDRESS_ENV} should be valid IPAddr"));
        let udp_port = std::env::var(UDP_PORT_ENV)
            .map(|port| {
                port.parse::<u16>()
                    .expect(&format!("{UDP_PORT_ENV} should be u16 integer"))
            })
            .expect(&format!("{UDP_PORT_ENV} env variable should be present"));

        let udp_address = SocketAddr::new(udp_ip, udp_port);

        let whip_token = std::env::var(WHIP_TOKEN_ENV)
            .expect(&format!("{WHIP_TOKEN_ENV} env variable should be present"));

        Config {
            ssl_config,
            session_command_sender: sender,
            udp_server_config: UDPServerConfig {
                address: udp_address,
            },
            tcp_server_config: TCPServerConfig {
                whip_token,
                address: tcp_address,
            },
        }
    }
}

pub fn get_global_config() -> &'static Config {
    GLOBAL_CONFIG.get().unwrap()
}

pub struct TCPServerConfig {
    pub address: SocketAddr,
    pub whip_token: String,
}

pub struct UDPServerConfig {
    pub address: SocketAddr,
}
