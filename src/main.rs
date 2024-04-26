use std::collections::HashMap;
use std::env;
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use dg_xch_core::blockchain::sized_bytes::{Bytes32, Bytes48};
use clap::{Parser};
use dg_xch_cli::wallets::plotnft_utils::{get_plotnft_by_launcher_id, scrounge_for_plotnfts};
use dg_xch_clients::ClientSSLConfig;
use dg_xch_clients::rpc::full_node::FullnodeClient;
use dg_xch_core::config::PoolWalletConfig;
use dg_xch_core::consensus::constants::CONSENSUS_CONSTANTS_MAP;
use dg_xch_core::ssl::create_all_ssl;
use dg_xch_keys::{key_from_mnemonic, master_sk_to_farmer_sk, master_sk_to_pool_sk, master_sk_to_pooling_authentication_sk, master_sk_to_singleton_owner_sk, master_sk_to_wallet_sk, master_sk_to_wallet_sk_unhardened};
use dg_xch_puzzles::clvm_puzzles::launcher_id_to_p2_puzzle_hash;
use dg_xch_puzzles::p2_delegated_puzzle_or_hidden_puzzle::puzzle_hash_for_pk;
use dialoguer::Confirm;
use home::home_dir;
use log::{info, LevelFilter, warn};
use simple_logger::SimpleLogger;
use tokio::fs::create_dir_all;
use crate::config::{Config, FarmingInfo, GigahorseHarvesterConfig};
use crate::prompts::{prompt_for_farming_fullnode, prompt_for_farming_port, prompt_for_launcher_id, prompt_for_mnemonic, prompt_for_payout_address, prompt_for_plot_directories, prompt_for_rpc_fullnode, prompt_for_rpc_port, prompt_for_ssl_path};

mod prompts;
mod config;

pub static PRIVATE_CRT: &str = "farmer/private_farmer.crt";
pub static PRIVATE_KEY: &str = "farmer/private_farmer.key";
pub static CA_PRIVATE_CRT: &str = "ca/private_ca.crt";

#[tokio::main]
async fn main() -> Result<(), Error> {
    SimpleLogger::new()
        .with_colors(true)
        .with_level(LevelFilter::Info)
        .env()
        .init()
        .unwrap_or_default();
    let cli = Cli::parse();
    let config_path = if let Some(s) = &cli.config {
        PathBuf::from(s)
    } else if let Ok(s) = env::var("CONFIG_PATH") {
        PathBuf::from(s)
    } else {
        let config_path = get_config_path();
        if let Some(parent) = config_path.parent() {
            create_dir_all(parent).await?;
        }
        config_path
    };
    generate_config_from_mnemonic(GenerateConfig {
        output_path: Some(config_path),
        mnemonic_file: cli.mnemonic_file,
        fullnode_ws_host: cli.fullnode_ws_host,
        fullnode_ws_port: cli.fullnode_ws_port,
        fullnode_rpc_host: cli.fullnode_rpc_host,
        fullnode_rpc_port: cli.fullnode_rpc_port,
        fullnode_ssl: cli.fullnode_ssl,
        network: cli.network,
        launcher_id: cli.launcher_id.map(Bytes32::from),
        payout_address: cli.payout_address,
        plot_directories: cli.plot_directories,
        additional_headers: None,
    })
        .await?;
    Ok(())
}

pub(crate) fn get_root_path() -> PathBuf {
    let prefix = home_dir().unwrap_or(Path::new("/").to_path_buf());
    prefix.as_path().join(Path::new(".config/fast_farmer/"))
}

pub(crate) fn get_ssl_root_path(config: &Config) -> PathBuf {
    if let Some(ssl_root_path) = &config.ssl_root_path {
        PathBuf::from(ssl_root_path)
    } else {
        get_root_path().as_path().join(Path::new("ssl/"))
    }
}

pub(crate) fn get_config_path() -> PathBuf {
    get_root_path()
        .as_path()
        .join(Path::new("fast_farmer.yaml"))
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<String>,
    #[arg(short = 'f', long)]
    fullnode_ws_host: Option<String>,
    #[arg(short = 'p', long)]
    fullnode_ws_port: Option<u16>,
    #[arg(short = 'r', long)]
    fullnode_rpc_host: Option<String>,
    #[arg(short = 'o', long)]
    fullnode_rpc_port: Option<u16>,
    #[arg(short = 's', long)]
    fullnode_ssl: Option<String>,
    #[arg(short = 'n', long)]
    network: Option<String>,
    #[arg(short = 'a', long)]
    payout_address: Option<String>,
    #[arg(short = 'd', long = "plot-directory")]
    plot_directories: Option<Vec<String>>,
    #[arg(short = 'm', long)]
    mnemonic_file: Option<String>,
    #[arg(short = 'l', long)]
    launcher_id: Option<String>,
}

pub struct GenerateConfig {
    pub output_path: Option<PathBuf>,
    pub mnemonic_file: Option<String>,
    pub fullnode_ws_host: Option<String>,
    pub fullnode_ws_port: Option<u16>,
    pub fullnode_rpc_host: Option<String>,
    pub fullnode_rpc_port: Option<u16>,
    pub fullnode_ssl: Option<String>,
    pub network: Option<String>,
    pub launcher_id: Option<Bytes32>,
    pub payout_address: Option<String>,
    pub plot_directories: Option<Vec<String>>,
    pub additional_headers: Option<HashMap<String, String>>,
}

pub async fn generate_config_from_mnemonic(gen_settings: GenerateConfig) -> Result<Config, Error> {
    //Check for Existing Config and prompt for override
    if let Some(op) = &gen_settings.output_path {
        if op.exists()
            && !Confirm::new()
            .with_prompt(format!(
                "An existing config exists at {:?}, would you like to override it? (Y/N)",
                op
            ))
            .interact()
            .map_err(|e| {
                Error::new(
                    ErrorKind::Interrupted,
                    format!("Dialog Interrupted: {:?}", e),
                )
            })?
        {
            return Err(Error::new(ErrorKind::Interrupted, "User Canceled"));
        }
    }
    let mut config = Config::default();
    let network = gen_settings
        .network
        .map(|v| {
            if CONSENSUS_CONSTANTS_MAP.contains_key(&v) {
                v
            } else {
                "mainnet".to_string()
            }
        })
        .unwrap_or("mainnet".to_string());
    config.selected_network = network;
    //Prompt the User for Mnemonic to Generate needed Keys
    let master_key = key_from_mnemonic(&prompt_for_mnemonic(gen_settings.mnemonic_file)?)?;
    //Prompt for Payout Address, Will populate farmer and pool reward addresses
    config.payout_address = prompt_for_payout_address(gen_settings.payout_address)?.to_string();
    //Prompt for Node to connect the Farming Websocket
    config.fullnode_ws_host =
        prompt_for_farming_fullnode(gen_settings.fullnode_ws_host)?.to_string();
    //If the User is not using the community node, ask for RPC info.
    //This is used to update the status of the fullnode and to search for plot_nft info
    //If using the migrate functions of the farmer this is also where the push_tx call will go
    config.fullnode_rpc_host = if let Some(host) = gen_settings.fullnode_rpc_host {
        host
    } else if "chia-proxy.evergreenminer-prod.com" == config.fullnode_ws_host {
        "chia-proxy.evergreenminer-prod.com".to_string()
    } else {
        prompt_for_rpc_fullnode(None)?
    };
    //Farming Port, typically 443 for community and 8444 for local node
    config.fullnode_ws_port = if let Some(port) = gen_settings.fullnode_ws_port {
        port
    } else if "chia-proxy.evergreenminer-prod.com" == config.fullnode_ws_host {
        443
    } else if "localhost" == config.fullnode_ws_host {
        8444
    } else {
        prompt_for_farming_port(None)?
    };
    //RPC Port, typically 443 for community and 8555 for local node
    config.fullnode_rpc_port = if let Some(port) = gen_settings.fullnode_rpc_port {
        port
    } else if "chia-proxy.evergreenminer-prod.com" == config.fullnode_rpc_host {
        443
    } else if "localhost" == config.fullnode_rpc_host {
        8555
    } else {
        prompt_for_rpc_port(None)?
    };
    //For community node this can be left blank as it will generate the required certs.
    //For local hosted nodes, it is recommended to create a folder and copy the "ssl/ca"
    //from your chia install to the created folder. This will allow FastFarmer to connect without
    //conflicting with the Chia Farmer that runs with the GUI or any other farmers.
    config.ssl_root_path = if "chia-proxy.evergreenminer-prod.com" == config.fullnode_ws_host {
        None
    } else {
        prompt_for_ssl_path(gen_settings.fullnode_ssl)?
    };
    //This tool is used to generate GigahorseCongigs.
    //For regular DruidGarden config please use the Open Source version of FastFarmer
    config.harvester_configs.gigahorse = Some(GigahorseHarvesterConfig {
        plot_directories: if let Some(dirs) = gen_settings.plot_directories {
            dirs
        } else {
            prompt_for_plot_directories()?
        },
        parallel_read: true,
        plot_search_depth: 2,
        max_cpu_cores: -1,
        max_cuda_devices: -1,
        max_opencl_devices: -1,
        cuda_device_list: vec![],
        opencl_device_list: vec![],
        recompute_host: "".to_string(),
        recompute_port: 0,
    });
    //Always check/generate the SSL, this will not overwrite existing files
    if let Some(ssl_path) = &config.ssl_root_path {
        create_all_ssl(Path::new(ssl_path), false)?;
    } else {
        let ssl_path = get_ssl_root_path(&config);
        create_all_ssl(&ssl_path, false)?;
        config.ssl_root_path = Some(ssl_path.to_string_lossy().to_string());
    }
    //If the RPC connection fails it is likely one of the below:
    //1. The wrong SSL path was given or the ca files don't match the Fullnode.
    //2. The self_hostname field in the Chia Fullnode config is not set to 0.0.0.0 and you are trying to connect remotely
    //3. The Port or Hostname fields are not set up correctly to match the Fullnode, verify which ports the RPC and WS are running on in the Chia config
    let client = rpc_client_from_config(&config, &gen_settings.additional_headers);
    let mut page = 0;
    let mut plotnfts = vec![];
    //Depending on how many "Claims" have happened with your PlotNFT this process can take some time.
    //
    if let Some(launcher_id) = prompt_for_launcher_id(gen_settings.launcher_id)? {
        info!("Searching for NFT with LauncherID: {launcher_id}");
        if let Some(plotnft) =
            get_plotnft_by_launcher_id(client.clone(), &launcher_id).await?
        {
            plotnfts.push(plotnft);
        } else {
            return Err(Error::new(
                ErrorKind::NotFound,
                "Failed to find a plotNFT with LauncherID: {launcher_id}",
            ));
        }
    } else {
        info!("No LauncherID Specified, Searching for PlotNFTs...");
        while page < 50 && plotnfts.is_empty() {
            let mut puzzle_hashes = vec![];
            for index in page * 50..(page + 1) * 50 {
                let wallet_sk =
                    master_sk_to_wallet_sk_unhardened(&master_key, index).map_err(|e| {
                        Error::new(
                            ErrorKind::InvalidInput,
                            format!("Failed to parse Wallet SK: {:?}", e),
                        )
                    })?;
                let pub_key: Bytes48 = wallet_sk.sk_to_pk().to_bytes().into();
                puzzle_hashes.push(puzzle_hash_for_pk(&pub_key)?);
                let hardened_wallet_sk =
                    master_sk_to_wallet_sk(&master_key, index).map_err(|e| {
                        Error::new(
                            ErrorKind::InvalidInput,
                            format!("Failed to parse Wallet SK: {:?}", e),
                        )
                    })?;
                let pub_key: Bytes48 = hardened_wallet_sk.sk_to_pk().to_bytes().into();
                puzzle_hashes.push(puzzle_hash_for_pk(&pub_key)?);
            }
            plotnfts.extend(scrounge_for_plotnfts(client.clone(), &puzzle_hashes).await?);
            page += 1;
        }
    }
    for plot_nft in plotnfts {
        config.pool_info.push(PoolWalletConfig {
            launcher_id: plot_nft.launcher_id,
            pool_url: plot_nft.pool_state.pool_url.unwrap_or_default(),
            target_puzzle_hash: plot_nft.pool_state.target_puzzle_hash,
            payout_instructions: config.payout_address.clone(),
            p2_singleton_puzzle_hash: launcher_id_to_p2_puzzle_hash(
                &plot_nft.launcher_id,
                plot_nft.delay_time as u64,
                &plot_nft.delay_puzzle_hash,
            )?,
            owner_public_key: plot_nft.pool_state.owner_pubkey,
            difficulty: None,
        });
        let mut owner_key = None;
        let mut auth_key = None;
        for i in 0..150 {
            let key = master_sk_to_singleton_owner_sk(&master_key, i).unwrap();
            let pub_key: Bytes48 = key.sk_to_pk().to_bytes().into();
            if pub_key == plot_nft.pool_state.owner_pubkey {
                let a_key = master_sk_to_pooling_authentication_sk(&master_key, i, 0).unwrap();
                owner_key = Some(key.into());
                auth_key = Some(a_key.into());
                break;
            }
        }
        if let Some(info) = config.farmer_info.iter_mut().find(|f| {
            if let Some(l) = &f.launcher_id {
                l == &plot_nft.launcher_id
            } else {
                false
            }
        }) {
            info.farmer_secret_key = master_sk_to_farmer_sk(&master_key)?.into();
            info.launcher_id = Some(plot_nft.launcher_id);
            info.pool_secret_key = Some(master_sk_to_pool_sk(&master_key)?.into());
            info.owner_secret_key = owner_key;
            info.auth_secret_key = auth_key;
        } else {
            config.farmer_info.push(FarmingInfo {
                farmer_secret_key: master_sk_to_farmer_sk(&master_key)?.into(),
                launcher_id: Some(plot_nft.launcher_id),
                pool_secret_key: Some(master_sk_to_pool_sk(&master_key)?.into()),
                owner_secret_key: owner_key,
                auth_secret_key: auth_key,
            });
        }
    }
    if config.farmer_info.is_empty() {
        warn!("No PlotNFT Found");
        config.farmer_info.push(FarmingInfo {
            farmer_secret_key: master_sk_to_farmer_sk(&master_key)?.into(),
            launcher_id: None,
            pool_secret_key: Some(master_sk_to_pool_sk(&master_key)?.into()),
            owner_secret_key: None,
            auth_secret_key: None,
        });
    }
    if let Some(op) = &gen_settings.output_path {
        config.save_as_yaml(op)?;
    }
    Ok(config)
}

pub(crate) fn rpc_client_from_config(
    config: &Config,
    headers: &Option<HashMap<String, String>>,
) -> Arc<FullnodeClient> {
    Arc::new(FullnodeClient::new(
        &config.fullnode_rpc_host,
        config.fullnode_rpc_port,
        600,
        if is_community_node(config) {
            None
        } else {
            config.ssl_root_path.clone().map(|s| ClientSSLConfig {
                ssl_crt_path: Path::new(&s)
                    .join(PRIVATE_CRT)
                    .to_string_lossy()
                    .to_string(),
                ssl_key_path: Path::new(&s)
                    .join(PRIVATE_KEY)
                    .to_string_lossy()
                    .to_string(),
                ssl_ca_crt_path: Path::new(&s)
                    .join(CA_PRIVATE_CRT)
                    .to_string_lossy()
                    .to_string(),
            })
        },
        headers,
    ))
}

pub fn is_community_node(config: &Config) -> bool {
    [
        "chia-proxy.evergreenminer-prod.com",
        "chia-proxy.galactechs.com",
        "chia-proxy-testnet11.evergreenminer-prod.com",
        "chia-proxy-testnet11.galactechs.com",
    ].contains(&config.fullnode_rpc_host.to_ascii_lowercase().trim())
}