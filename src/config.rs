use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::OnceLock;

use crate::acceptor::SSLConfig;

pub struct Config {
    pub ssl_config: SSLConfig,
    pub tcp_server_config: TCPServerConfig,
    pub udp_server_config: UDPServerConfig,
    pub rtc_event_sink_addr: SocketAddr,
}

const TCP_IP_ENV: &'static str = "TCP_ADDRESS";
const TCP_PORT_ENV: &'static str = "TCP_PORT";
const UDP_IP_ENV: &'static str = "UDP_ADDRESS";
const UDP_PORT_ENV: &'static str = "UDP_PORT";
const WHIP_TOKEN_ENV: &'static str = "WHIP_TOKEN";
const RTC_EVENT_SINK_ADDR: &'static str = "RTC_EVENT_SINK_ADDR";
const CERTS_DIR: &'static str = "CERTS_DIR";

impl Config {
    pub fn initialize() -> Self {
        // TCP server config
        let tcp_ip = IpAddr::from_str(
            &std::env::var(TCP_IP_ENV)
                .expect(&format!("{TCP_IP_ENV} env variable should be present")),
        )
        .expect(&format!("${TCP_IP_ENV} should be valid IPAddr"));
        let tcp_port = std::env::var(TCP_PORT_ENV)
            .map(|port| {
                port.parse::<u16>()
                    .expect(&format!("{TCP_PORT_ENV} should be u16 integer"))
            })
            .expect(&format!("{TCP_PORT_ENV} env variable should be present"));

        let tcp_address = SocketAddr::new(tcp_ip, tcp_port);

        // UDP server config
        let udp_ip = IpAddr::from_str(
            &std::env::var(UDP_IP_ENV)
                .expect(&format!("{UDP_IP_ENV} env variable should be present")),
        )
        .expect(&format!("${UDP_IP_ENV} should be valid IPAddr"));

        let udp_port = std::env::var(UDP_PORT_ENV)
            .map(|port| {
                port.parse::<u16>()
                    .expect(&format!("{UDP_PORT_ENV} should be u16 integer"))
            })
            .expect(&format!("{UDP_PORT_ENV} env variable should be present"));

        let udp_address = SocketAddr::new(udp_ip, udp_port);

        let whip_token = std::env::var(WHIP_TOKEN_ENV)
            .expect(&format!("{WHIP_TOKEN_ENV} env variable should be present"));

        let rtc_event_sink_addr = std::env::var(RTC_EVENT_SINK_ADDR).unwrap();

        // Configurable directories
        let certs_dir = PathBuf::from(std::env::var(CERTS_DIR).unwrap());

        let ssl_config = SSLConfig::new(certs_dir);

        Config {
            ssl_config,
            udp_server_config: UDPServerConfig {
                address: udp_address,
            },
            tcp_server_config: TCPServerConfig {
                whip_token,
                address: tcp_address,
            },
            rtc_event_sink_addr: SocketAddr::from_str(&rtc_event_sink_addr).unwrap(),
        }
    }
}

static GLOBAL_CONFIG: OnceLock<Config> = OnceLock::new();

pub fn get_global_config() -> &'static Config {
    GLOBAL_CONFIG.get_or_init(Config::initialize)
}

pub struct TCPServerConfig {
    pub address: SocketAddr,
    pub whip_token: String,
}

pub struct UDPServerConfig {
    pub address: SocketAddr,
}
