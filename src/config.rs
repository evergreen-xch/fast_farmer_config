use dg_xch_core::blockchain::sized_bytes::{Bytes32};
use dg_xch_core::config::PoolWalletConfig;
use std::fs;
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};

const fn default_true() -> bool {
    true
}
const fn default_0() -> i64 {
    0
}
const fn default_metrics_port() -> u16 {
    8080
}
const fn default_neg_1() -> i32 {
    -1
}
const fn default_recompute() -> u16 {
    0
}
const fn default_none<T>() -> Option<T> {
    None
}

#[derive(Default, Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FarmingInfo {
    pub farmer_secret_key: Bytes32,
    pub launcher_id: Option<Bytes32>,
    pub pool_secret_key: Option<Bytes32>,
    pub owner_secret_key: Option<Bytes32>,
    pub auth_secret_key: Option<Bytes32>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MetricsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_metrics_port")]
    pub port: u16,
}
impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 8080,
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DruidGardenHarvesterConfig {
    #[serde(default = "Vec::new")]
    pub plot_directories: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GigahorseHarvesterConfig {
    #[serde(default = "Vec::new")]
    pub plot_directories: Vec<String>,
    #[serde(default = "default_true")]
    pub parallel_read: bool,
    #[serde(default = "default_0")]
    pub plot_search_depth: i64,
    #[serde(default = "default_neg_1")]
    pub max_cpu_cores: i32,
    #[serde(default = "default_neg_1")]
    pub max_cuda_devices: i32,
    #[serde(default = "default_neg_1")]
    pub max_opencl_devices: i32,
    #[serde(default = "Vec::new")]
    pub cuda_device_list: Vec<u8>,
    #[serde(default = "Vec::new")]
    pub opencl_device_list: Vec<u8>,
    #[serde(default = "String::new")]
    pub recompute_host: String,
    #[serde(default = "default_recompute")]
    pub recompute_port: u16,
}
impl Default for GigahorseHarvesterConfig {
    fn default() -> Self {
        Self {
            plot_directories: vec![],
            parallel_read: true,
            plot_search_depth: 0,
            max_cpu_cores: -1,
            max_cuda_devices: -1,
            max_opencl_devices: -1,
            cuda_device_list: vec![],
            opencl_device_list: vec![],
            recompute_host: String::new(),
            recompute_port: 0,
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HarvesterConfig {
    #[serde(default = "default_none")]
    pub druid_garden: Option<DruidGardenHarvesterConfig>,
    #[serde(default = "default_none")]
    pub gigahorse: Option<GigahorseHarvesterConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Config {
    pub selected_network: String,
    pub ssl_root_path: Option<String>,
    pub fullnode_ws_host: String,
    pub fullnode_ws_port: u16,
    pub fullnode_rpc_host: String,
    pub fullnode_rpc_port: u16,
    pub farmer_info: Vec<FarmingInfo>,
    pub pool_info: Vec<PoolWalletConfig>,
    pub payout_address: String,
    pub harvester_configs: HarvesterConfig,
    pub metrics: Option<MetricsConfig>,
}
impl Config {
    pub fn save_as_yaml<P: AsRef<Path>>(&self, path: P) -> Result<(), Error> {
        fs::write(
            path.as_ref(),
            serde_yaml::to_string(&self)
                .map_err(|e| Error::new(ErrorKind::Other, format!("{:?}", e)))?,
        )
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            selected_network: "mainnet".to_string(),
            ssl_root_path: None,
            fullnode_rpc_host: "localhost".to_string(),
            fullnode_rpc_port: 8555,
            fullnode_ws_host: "localhost".to_string(),
            fullnode_ws_port: 8444,
            farmer_info: vec![],
            pool_info: vec![],
            payout_address: "".to_string(),
            harvester_configs: HarvesterConfig {
                druid_garden: None,
                gigahorse: Some(GigahorseHarvesterConfig::default()),
            },
            metrics: Some(MetricsConfig {
                enabled: true,
                port: 8080,
            }),
        }
    }
}
impl TryFrom<&Path> for Config {
    type Error = Error;
    fn try_from(value: &Path) -> Result<Self, Self::Error> {
        serde_yaml::from_str::<Config>(&fs::read_to_string(value)?)
            .map_err(|e| Error::new(ErrorKind::Other, format!("{:?}", e)))
    }
}
impl TryFrom<&PathBuf> for Config {
    type Error = Error;
    fn try_from(value: &PathBuf) -> Result<Self, Self::Error> {
        Self::try_from(value.as_path())
    }
}