mod spl;
mod dex;
mod cli;
mod api;

mod utils;

use chrono::Local;
use env_logger::Builder;
use std::io::Write;
use std::path::Path;
use clap::Parser;
use log::{error, info};
use solana_client::rpc_client::RpcClient;
use solana_sdk::signature::Keypair;
use config_file::FromConfigFile;
use solana_sdk::genesis_config::ClusterType;
use crate::cli::args::{CliArgs, Commands};
use crate::cli::config::{Config, ProjectConfig};


#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() {
    let args: CliArgs = CliArgs::parse();

    let mut log_builder = Builder::new();
    log_builder.format(|buf, record| {
        writeln!(buf,
                 "{} [{}] - {}",
                 Local::now().format("%Y-%m-%d %H:%M:%S"),
                 record.level(),
                 record.args()
        )
    });

    if args.verbose {
        log_builder.filter(Some("api"), log::LevelFilter::Debug);
        log_builder.filter(Some("spl"), log::LevelFilter::Debug);
        log_builder.filter(Some("dex"), log::LevelFilter::Debug);
        log_builder.filter(Some("api"), log::LevelFilter::Debug);
        log_builder.filter(Some("utils"), log::LevelFilter::Debug);
    } else {
        log_builder.filter_level(log::LevelFilter::Info);
    }

    log_builder.init();


    let mut project_empty = false;
    if args.name.is_none() {
        match args.command {
            Commands::Buy { .. } => {
                project_empty = true;
            },
            Commands::Sell { .. } => {
                project_empty = true;
            },
            Commands::PoolInformation { .. } => {
                project_empty = true;
            },
            _ => {}
        }
    }

    let config: Config = match Config::from_config_file(&args.config) {
        Ok(c) => c,
        Err(e) => {
            panic!("Error reading config file: {:?}", e);
        }
    };

    let rpc_client = RpcClient::new(config.rpc_url.clone());
    let keypair = Keypair::from_base58_string(&config.wallet_keypair);

    let cluster_type = if args.dev {
        ClusterType::Devnet
    } else {
        ClusterType::MainnetBeta
    };

    info!("Cluster type: {:?}", cluster_type);

    if project_empty {
        match args.command {
            Commands::Buy { mint, quote_mint, amount, wait, skip } => {
                cli::buy(
                    &rpc_client,
                    &config,
                    &keypair,
                    &mint,
                    &quote_mint,
                    amount,
                    wait,
                    skip,
                    cluster_type
                ).await;
                return;
            }

            Commands::Sell { mint, quote_mint, percent, skip } => {
                cli::sell(
                    &rpc_client,
                    &keypair,
                    &mint,
                    &quote_mint,
                    percent,
                    skip,
                    cluster_type
                ).await;
                return;
            }

            Commands::PoolInformation { ref mint, ref quote_mint } => {
                cli::get_pool_information(
                    &config,
                    &mint,
                    &quote_mint,
                    cluster_type
                ).await;
                return;
            }
            _ => {
                info!("Project name is required for this command");
                return;
            }
        }
    }


    // load project directory
    let project_dir = format!("{}/{}", config.project_directory, args.name.unwrap());
    info!("Project directory: {:?}", project_dir);


    let project_config_file = format!("{}/config.yaml", project_dir);
    let mut has_project_config = true;
    if !Path::new(&project_config_file).exists() {
        has_project_config = false;
    };

    let mut project_config: ProjectConfig = Default::default();
    let mut project_image: String = "".to_string();

    if has_project_config {
        project_config = match ProjectConfig::from_config_file(&project_config_file) {
            Ok(c) => c,
            Err(e) => {
                error!("Error reading project config file: {:?}", e);
                return;
            }
        };

        project_image = format!("{}/{}", project_dir, project_config.image_filename);
        if !Path::new(&project_image).exists() {
            error!("Project image not found");
            return;
        }
    }

    let mut token_created = true;
    if project_config.token_keypair.is_empty() {
        token_created = false;
    }

    let project_metadata = format!("{}/metadata.json", project_dir);
    let mut has_metadata = true;
    if project_config.metadata_uri.is_empty() {
        has_metadata = false;
    }

    let project_market = format!("{}/market.yaml", project_dir);
    let mut has_market = true;
    if !Path::new(&project_market).exists() {
        has_market = false;
    }

    let project_liquidity = format!("{}/liquidity.yaml", project_dir);
    let mut has_liquidity = true;
    if !Path::new(&project_liquidity).exists() {
        has_liquidity = false;
    }

    match args.command {
        Commands::Create {
            generate_wallet,
            count,
            airdrop,
            percentage
        } => {
            cli::create_token(
                &rpc_client,
                &keypair,
                project_dir.clone(),
                &config,
                &mut project_config,
                project_config_file,
                project_image,
                project_metadata,
                has_project_config,
                has_metadata,
                generate_wallet,
                count,
                airdrop,
                percentage
            ).await;
            return;
        }
        _ => {}
    }

    if token_created == false {
        info!("Token not created");
        return;
    }

    match args.command {
        Commands::Airdrop {
            percentage
        } => {
            cli::airdrop(
                &rpc_client,
                &keypair,
                project_dir,
                &mut project_config,
                percentage,
                has_project_config
            ).await;
        },

        Commands::Withdraw { destination } => {
            cli::withdraw(
                &rpc_client,
                &keypair,
                &project_config,
                destination
            ).await;
        },

        Commands::Burn {
            percentage, mint, airdrop, single, pay, liquidity
        } => {
            cli::burn(
                &rpc_client,
                &keypair,
                &project_config,
                &project_liquidity,
                &mint,
                percentage,
                airdrop,
                single,
                pay,
                liquidity
            ).await;
        }

        Commands::Market {
            quote_mint,
            event_queue_length,
            request_queue_length,
            orderbook_length
        } => {
            if token_created == false {
                info!("Token not created");
                return;
            }

            cli::create_market(
                &rpc_client,
                &keypair,
                project_dir,
                &project_config,
                quote_mint,
                event_queue_length,
                request_queue_length,
                orderbook_length,
                cluster_type,
                has_market
            ).await;
        }

        Commands::AddLiquidity { amount } => {
            if token_created == false {
                info!("Token not created");
                return;
            }

            cli::add_liquidity(
                &rpc_client,
                &keypair,
                project_dir,
                &project_config,
                project_market,
                project_liquidity,
                amount,
                cluster_type,
                has_market,
                has_liquidity
            ).await;
        },

        Commands::RemoveLiquidity {} => {
            if token_created == false {
                info!("Token not created");
                return;
            }

            cli::remove_liquidity(
                &rpc_client,
                &keypair,
                project_dir,
                project_market,
                cluster_type,
                has_market,
                has_liquidity
            ).await;
        },

        Commands::ProjectSell { mint, percent, sell_all, wallet_count, interval } => {
            if token_created == false {
                info!("Token not created");
                return;
            }

            cli::project_sell(
                &rpc_client,
                &config,
                &project_config,
                &mint,
                sell_all,
                wallet_count,
                percent,
                interval,
                cluster_type,
                has_market,
                has_liquidity,
                has_project_config
            ).await;
        },

        Commands::AutoSell { mint, quote_mint, interval, percentage } => {
            if token_created == false {
                info!("Token not created");
                return;
            }

            cli::auto_sell(
                &rpc_client,
                &config,
                &project_config,
                &mint,
                &quote_mint,
                interval,
                percentage,
                cluster_type
            ).await;
        }
        _ => {}
    }
}