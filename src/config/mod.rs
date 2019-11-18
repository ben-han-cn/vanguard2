use crate::error::VgError;
use serde::{Deserialize, Serialize};
use std::{fs::File, io::prelude::*, path::Path};

#[derive(Debug, Deserialize, Serialize)]
pub struct VanguardConfig {
    pub server: ServerConfig,
    pub auth: AuthorityConfig,
    pub recursor: RecursorConfig,
    pub forwarder: ForwarderConfig,
    pub vg_ctrl: VgCtrlConfig,
    pub metrics: MetricsConfig,
}

impl VanguardConfig {
    pub fn load_config<P: AsRef<Path>>(path: P) -> Result<Self, VgError> {
        let path = path.as_ref();
        let mut file = File::open(path)?;
        let mut config_string = String::new();
        file.read_to_string(&mut config_string)?;
        let config = serde_yaml::from_str(&config_string)?;
        Ok(config)
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct ServerConfig {
    pub address: String,
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
#[serde(default)]
pub struct AuthorityConfig {
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
#[serde(default)]
pub struct RecursorConfig {
    pub enable: bool,
}

impl Default for RecursorConfig {
    fn default() -> Self {
        RecursorConfig { enable: true }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct ForwarderConfig {
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
#[serde(default)]
pub struct VgCtrlConfig {
    pub address: String,
}

impl Default for VgCtrlConfig {
    fn default() -> Self {
        VgCtrlConfig {
            address: "127.0.0.1:5555".to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
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
