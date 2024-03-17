use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "spl-token-creator", about = "SPL token management")]
#[command(version, long_about = None)]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Commands,

    /// Project name
    #[arg(short = 'n', long)]
    pub name: Option<String>,

    /// Config file
    #[arg(short = 'c', long, default_value = "config.yaml")]
    pub config: String,

    /// Use devnet program ids
    #[arg(long, default_value_t = false)]
    pub dev: bool,

    /// verbose log
    #[arg(long, default_value_t = false)]
    pub verbose: bool,

    /// Custom Keypair (base58) file (ex. wallet.yaml)
    #[arg(long)]
    pub keypair: Option<String>
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Create a new SPL token
    Create {
        /// Generate wallets
        #[arg(long, short = 'g')]
        generate_wallet: bool,

        /// Wallet generation count
        #[arg(long, default_value_t = 9)]
        count: i32,

        /// Automatically distribute tokens (airdrop) on creation
        #[arg(long, short = 'a', default_value_t = false)]
        airdrop: bool,

        /// percentage amount to distribute to each wallet
        #[arg(short = 'p', default_value_t = 50.0)]
        percentage: f64,
    },

    /// Generate Wallets for Project
    GenerateWallet {
        /// Wallet generation count
        #[arg(short='c', long)]
        count: i32,

        /// Replace current wallets in the project
        #[arg(long, default_value_t = false)]
        replace: bool,
    },

    /// Open an Opendex Market Listing
    Market {
        /// Quote mint
        #[arg(long, short = 'q', default_value = "So11111111111111111111111111111111111111112")]
        quote_mint: String,

        /// Event Queue Length
        #[arg(long, short = 'e', default_value_t = 128)]
        event_queue_length: u64,

        /// Request Queue Length
        #[arg(long, short = 'r', default_value_t = 63)]
        request_queue_length: u64,

        /// Orderbook Length
        #[arg(long, short = 'o', default_value_t = 201)]
        orderbook_length: u64,
    },

    /// Airdrop SPL token to generated wallets
    Airdrop {
        /// percentage amount to distribute to each wallet
        #[arg(short = 'p', default_value_t = 50.0)]
        percentage: f64,
    },

    /// Burn all SPL token
    Burn {
        /// burn token percentage
        #[arg(short = 'p', default_value_t = 100.0)]
        percentage: f64,

        /// Token Mint
        #[arg(short = 'm', default_value = "So11111111111111111111111111111111111111112")]
        mint: String,

        /// This will burn all the airdropped tokens
        #[arg(long, default_value_t = true)]
        airdrop: bool,

        /// This will burn tokens in the payer account
        #[arg(long, default_value_t = false)]
        single: bool,

        /// This will burn tokens paid by payer
        #[arg(long, default_value_t = false)]
        pay: bool,

        /// Burn liquidity
        #[arg(long, default_value_t = false)]
        liquidity: bool
    },

    /// Add liquidity
    AddLiquidity {
        #[arg(short = 's')]
        amount: f64,
    },

    /// Remove liquidity
    RemoveLiquidity {},

    /// Get Market State
    PoolInformation {
        /// Token Mint
        #[arg(short = 'm')]
        mint: String,

        /// Quote Mint
        #[arg(short = 'q', default_value = "So11111111111111111111111111111111111111112")]
        quote_mint: String,
    },

    /// Buy Token
    Buy {
        /// Token Mint
        #[arg(short = 'm', default_value = "So11111111111111111111111111111111111111112")]
        mint: String,

        /// Quote Mint
        #[arg(short = 'q', default_value = "So11111111111111111111111111111111111111112")]
        quote_mint: String,

        /// SOL Amount
        #[arg(short = 'a', default_value_t = 0.001)]
        amount: f64,

        /// Wait for pool
        #[arg(short = 'w', default_value_t = false)]
        wait: bool,

        /// Skip WSOL Account creation
        #[arg(long, default_value_t = false)]
        skip: bool,
    },

    /// Sell Token
    Sell {
        /// Token Mint
        #[arg(short = 'm', default_value = "So11111111111111111111111111111111111111112")]
        mint: String,

        /// Quote Mint
        #[arg(short = 'q', default_value = "So11111111111111111111111111111111111111112")]
        quote_mint: String,

        /// Token percentage to sell
        #[arg(short = 'p', default_value_t = 100.0)]
        percent: f64,

        /// Skip WSOL Account creation
        #[arg(long, default_value_t = true)]
        skip: bool,
    },

    /// Withdraw all SOL from generated accounts
    Withdraw {
        /// Destination
        #[arg(short = 'd')]
        destination: Option<String>,
    },

    /// Sell Project Token
    ProjectSell {
        /// Token Mint
        #[arg(short = 'm', default_value = "So11111111111111111111111111111111111111112")]
        mint: String,

        /// Percent to sell
        #[arg(short = 'a', default_value_t = 100.0)]
        percent: f64,

        /// Sell all wallets
        #[arg(long, default_value_t = false)]
        sell_all: bool,

        /// Wallet count in the project to be sold
        #[arg(short = 'c', default_value_t = 1)]
        wallet_count: i32,

        /// Sell interval
        #[arg(short = 'i', default_value_t = 1.0)]
        interval: f64
    },

    /// Auto sell the airdropped token when liquidity pool is added
    AutoSell {
        /// Token Mint
        #[arg(short = 'm', default_value = "So11111111111111111111111111111111111111112")]
        mint: String,

        /// Quote Mint
        #[arg(short = 'q', default_value = "So11111111111111111111111111111111111111112")]
        quote_mint: String,

        /// Sell interval
        #[arg(short = 'i', default_value_t = 1.0)]
        interval: f64,

        /// Sell percentage
        #[arg(short = 'p', default_value_t = 100.0)]
        percentage: f64,
    }
}