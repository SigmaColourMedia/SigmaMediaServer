use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::OnceLock;

use crate::acceptor::SSLConfig;

pub struct Config {
    pub ssl_config: SSLConfig,
    pub tcp_server_config: TCPServerConfig,
    pub notification_bus_config: NotificationBusConfig,
    pub udp_server_config: UDPServerConfig,
    pub file_storage_config: FileStorageConfig,
    pub frontend_url: String,
}

const TCP_ADDRESS_ENV: &'static str = "TCP_ADDRESS";
const TCP_PORT_ENV: &'static str = "TCP_PORT";
const UDP_ADDRESS_ENV: &'static str = "UDP_ADDRESS";
const UDP_PORT_ENV: &'static str = "UDP_PORT";
const WHIP_TOKEN_ENV: &'static str = "WHIP_TOKEN";
const NOTIFICATION_BUS_ADDRESS_ENV: &'static str = "NOTIFICATION_BUS_ADDRESS";
const NOTIFICATION_BUS_PORT_ENV: &'static str = "NOTIFICATION_BUS_PORT";
const FILE_STORAGE_ADDRESS_ENV: &'static str = "FILE_STORAGE_ADDRESS";
const FILE_STORAGE_PORT_ENV: &'static str = "FILE_STORAGE_PORT";
const FRONTEND_URL_ENV: &'static str = "FRONTEND_URL";

impl Config {
    pub fn initialize() -> Self {
        let ssl_config = SSLConfig::new();

        // TCP server config
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

        // UDP server config
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

        // NotificationBus config
        let notification_bus_ip = IpAddr::from_str(
            &std::env::var(NOTIFICATION_BUS_ADDRESS_ENV)
                .expect(&format!("{UDP_ADDRESS_ENV} env variable should be present")),
        )
        .expect(&format!("${UDP_ADDRESS_ENV} should be valid IPAddr"));

        let notification_bus_port = std::env::var(NOTIFICATION_BUS_PORT_ENV)
            .map(|port| {
                port.parse::<u16>()
                    .expect(&format!("{UDP_PORT_ENV} should be u16 integer"))
            })
            .expect(&format!("{UDP_PORT_ENV} env variable should be present"));

        let notification_bus_address = SocketAddr::new(notification_bus_ip, notification_bus_port);

        // File storage config
        let file_storage_ip = IpAddr::from_str(
            &std::env::var(FILE_STORAGE_ADDRESS_ENV)
                .expect(&format!("{UDP_ADDRESS_ENV} env variable should be present")),
        )
        .expect(&format!("${UDP_ADDRESS_ENV} should be valid IPAddr"));

        let file_storage_port = std::env::var(FILE_STORAGE_PORT_ENV)
            .map(|port| {
                port.parse::<u16>()
                    .expect(&format!("{UDP_PORT_ENV} should be u16 integer"))
            })
            .expect(&format!("{UDP_PORT_ENV} env variable should be present"));

        let file_storage_address = SocketAddr::new(file_storage_ip, file_storage_port);

        // Frontend URL
        let frontend_url =
            std::env::var(FRONTEND_URL_ENV).expect("FRONTEND_URL env should be defined");

        Config {
            ssl_config,
            udp_server_config: UDPServerConfig {
                address: udp_address,
            },
            tcp_server_config: TCPServerConfig {
                whip_token,
                address: tcp_address,
            },
            notification_bus_config: NotificationBusConfig {
                address: notification_bus_address,
            },
            file_storage_config: FileStorageConfig {
                address: file_storage_address,
            },
            frontend_url,
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

pub struct NotificationBusConfig {
    pub address: SocketAddr,
}

pub struct FileStorageConfig {
    pub address: SocketAddr,
}
