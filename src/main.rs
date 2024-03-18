mod spl;
mod dex;
mod cli;
mod api;

mod utils;

use std::io;
use chrono::Local;
use env_logger::Builder;
use std::io::Write;
use std::path::Path;
use std::str::FromStr;
use clap::Parser;
use log::{debug, error, info};
use solana_client::rpc_client::RpcClient;
use solana_sdk::signature::{Keypair, Signer};
use config_file::{FromConfigFile};
use solana_program::pubkey::Pubkey;
use solana_sdk::genesis_config::ClusterType;
use crate::api::dexscreener::DexScreener;
use crate::cli::args::{CliArgs, Commands};
use crate::cli::config::{Config, ProjectConfig, WalletFile};


#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() {
    let mut args: CliArgs = CliArgs::parse();

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

    args.name = match args.name {
        Some(name) => Some(name.to_lowercase()),
        None => None
    };

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
            Commands::GenerateProject { .. } => {
                project_empty = true;
            },
            Commands::Monitor { .. } => {
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
    let mut keypair = Keypair::from_base58_string(&config.wallet_keypair);

    if args.keypair.is_some() {
        match WalletFile::from_config_file(
            &args.keypair.unwrap()
        ) {
            Ok(a) => {
                keypair = Keypair::from_base58_string(&a.key)
            }

            Err(e) => {
                error!("Error reading keypair file: {:?}", e);
                return;
            }
        };
    }

    let cluster_type = if args.dev {
        ClusterType::Devnet
    } else {
        ClusterType::MainnetBeta
    };

    info!("Cluster type: {:?}", cluster_type);

    if project_empty {
        match args.command {
            Commands::Monitor { ref mint } => {
                tokio::task::block_in_place(|| {
                    let dex_screener = DexScreener::new();
                    let mut token_price = 0.0;
                    let mut initial_price = 0.0;

                    info!("Initial Price {} SOL", initial_price);
                    info!("Waiting for price change");
                    loop {
                        let k = dex_screener.get_token_price(&mint);
                        match k {
                            Ok(k) => {
                                if token_price < k || token_price > k {
                                    if initial_price == 0f64.powi(9) {
                                        info!("Took Initial Price");
                                        initial_price = k;
                                    }

                                    token_price = k;
                                    info!("Price changed!");
                                    info!("1 Token = {:.9} SOL Price", token_price);
                                    info!("Profit: {}", token_price - initial_price);
                                    info!("Percent Increase: {}", ((token_price - initial_price) / initial_price) * 100f64);
                                }
                            }
                            Err(e) => {
                                debug!("Error: {}", e);
                            }
                        }
                        std::thread::sleep(std::time::Duration::from_millis(1000 / (300 / 60)));
                    }
                });
            },
            Commands::GenerateProject { name, symbol, icon, description, mint, decimal } => {
                println!("Generating project files");

                let name = name.unwrap_or_else(|| {
                    print!("Enter Token Name: ");
                    io::stdout().flush().unwrap();

                    let mut input = String::new();
                    io::stdin().read_line(&mut input)
                               .expect("Invalid Input");
                    input.trim().to_string()
                });

                let symbol = symbol.unwrap_or_else(|| {
                    print!("Enter Token Symbol: ");
                    io::stdout().flush().unwrap();

                    let mut input = String::new();
                    io::stdin().read_line(&mut input)
                               .expect("Invalid Input");

                    input.trim().to_string()
                });

                let icon = icon.unwrap_or_else(|| {
                    print!("Enter Icon Name [eg. icon.jpg]: ");
                    io::stdout().flush().unwrap();

                    let mut input = String::new();
                    io::stdin().read_line(&mut input)
                               .expect("Invalid Input");
                    input.trim().to_string()
                });

                let description_file_path = description.unwrap_or_else(|| {
                    print!("Enter Description File Location [eg. description.txt]: ");
                    io::stdout().flush().unwrap();
                    let mut input = String::new();
                    io::stdin().read_line(&mut input)
                               .expect("Invalid Input");
                    input.trim().to_string()
                });

                let contents = std::fs::read_to_string(description_file_path)
                    .expect("Should have been able to read the file");

                println!();
                println!("Project Name: {}", &name);
                println!("Project Symbol: {}", &symbol);
                println!("Icon Path: {}", &icon);
                println!("Description: {}", &contents);

                let project_dir = format!("{}/{}", config.project_directory, &name.to_lowercase());
                println!("Project Directory: {}", project_dir);

                match std::fs::create_dir(project_dir.clone()) {
                    Ok(_) => {}
                    Err(_) => {
                        error!("Project directory already exists");
                        return;
                    }
                };

                let project_config = ProjectConfig {
                    name: name.clone(),
                    symbol,
                    description: contents,
                    mint_amount: mint,
                    decimal,
                    image_filename: icon.clone(),
                    metadata_uri: "".to_string(),
                    token_keypair: "".to_string(),
                    wallets: vec![],
                    wsol_wallets: vec![],
                };

                let project_config_file = format!("{}/config.yaml", project_dir.clone());
                std::fs::write(
                    &project_config_file,
                    serde_yaml::to_string(&project_config).unwrap()
                ).expect("Failed to write project config file");

                println!();
                println!("NOTE: The icon ({}) you've added should be inside {}",
                         icon.clone(),
                         project_dir.clone());

                return;
            },

            Commands::Buy { mint, quote_mint, amount, wait, skip, overhead } => {
                cli::buy(
                    &rpc_client,
                    &config,
                    &keypair,
                    &mint,
                    &quote_mint,
                    amount,
                    wait,
                    skip,
                    overhead,
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
            _ => {}
        }
    }

    // load project directory
    if args.name.is_none() {
        info!("Project name is required for this command");
        return;
    }

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
            percentage, freeze
        } => {
            cli::create_token(
                &rpc_client,
                &keypair,
                &project_dir,
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
                percentage,
                freeze,
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
        Commands::RevokeAuthority {} => {
            cli::revoke_mint_authority(
                &rpc_client,
                &keypair,
                &project_config
            ).await;
        },
        Commands::GenerateWallet { count, replace } => {
            cli::generate_wallets(
                &project_config_file,
                &mut project_config,
                count,
                replace
            ).await;
        }

        Commands::Airdrop {
            percentage, confirm
        } => {
            cli::airdrop(
                &rpc_client,
                &keypair,
                project_dir,
                &mut project_config,
                percentage,
                has_project_config,
                confirm
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

        Commands::Balance {} => {
            cli::check_balance(
                &rpc_client,
                &project_config,
            );
        },

        Commands::CreateWsol {} => {
            cli::create_wsol(
                &rpc_client,
                &project_dir,
                &mut project_config,
            ).await;
        },

        Commands::AddLiquidity { amount } => {
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

        Commands::AutoSell { mint, quote_mint, overhead, interval, percentage, withdraw, destination } => {
            let mut dest = keypair.pubkey();
            if destination.is_some() {
                dest = Pubkey::from_str(&destination.unwrap()).unwrap()
            }

            cli::auto_sell(
                &rpc_client,
                &config,
                &project_config,
                &mint,
                &quote_mint,
                interval,
                overhead,
                percentage,
                withdraw,
                &dest,
                cluster_type
            ).await;
        }
        _ => {}
    }
}