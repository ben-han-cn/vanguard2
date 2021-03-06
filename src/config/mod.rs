use anyhow;
use serde::{Deserialize, Serialize};
use std::{fs::File, io::prelude::*, path::Path};

const DEFAULT_MESSAGE_CACHE_SIZE: usize = 10240;

#[derive(Default, Debug, Deserialize, Serialize)]
pub struct VanguardConfig {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub auth: AuthorityConfig,
    #[serde(default)]
    pub recursor: RecursorConfig,
    #[serde(default)]
    pub forwarder: ForwarderConfig,
    #[serde(default)]
    pub controller: ControllerConfig,
    #[serde(default)]
    pub metrics: MetricsConfig,
}

impl VanguardConfig {
    pub fn load_config<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let path = path.as_ref();
        let mut file = File::open(path)?;
        let mut config_string = String::new();
        file.read_to_string(&mut config_string)?;
        let config = serde_yaml::from_str(&config_string)?;
        Ok(config)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ServerConfig {
    pub address: String,
    #[serde(default)]
    pub enable_tcp: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            address: "0.0.0.0:53".to_string(),
            enable_tcp: false,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AuthorityConfig {
    #[serde(default)]
    pub zones: Vec<AuthZoneConfig>,
}

impl Default for AuthorityConfig {
    fn default() -> Self {
        AuthorityConfig { zones: Vec::new() }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AuthZoneConfig {
    pub name: String,
    pub file_path: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RecursorConfig {
    #[serde(default)]
    pub enable: bool,

    #[serde(default)]
    pub cache_size: usize,
}

impl Default for RecursorConfig {
    fn default() -> Self {
        RecursorConfig {
            enable: true,
            cache_size: DEFAULT_MESSAGE_CACHE_SIZE,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ForwarderConfig {
    #[serde(default)]
    pub forwarders: Vec<ZoneForwarderConfig>,
}

impl Default for ForwarderConfig {
    fn default() -> Self {
        ForwarderConfig {
            forwarders: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ZoneForwarderConfig {
    pub zone_name: String,
    pub addresses: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ControllerConfig {
    pub address: String,
}

impl Default for ControllerConfig {
    fn default() -> Self {
        ControllerConfig {
            address: "127.0.0.1:5556".to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MetricsConfig {
    pub address: String,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        MetricsConfig {
            address: "127.0.0.1:9100".to_string(),
        }
    }
}
