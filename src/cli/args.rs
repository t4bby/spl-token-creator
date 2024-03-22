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

    /// Verbose mode
    #[arg(long, default_value_t = false)]
    pub verbose: bool,

    /// Custom Keypair (base58) file (ex. wallet.yaml)
    #[arg(short = 'k', long)]
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
        #[arg(long, default_value_t = 1)]
        count: i32,

        /// Automatically distribute tokens (airdrop) on creation
        #[arg(long, short = 'a', default_value_t = false)]
        airdrop: bool,

        /// percentage amount to distribute to each wallet
        #[arg(short = 'p', default_value_t = 50.0)]
        percentage: f64,

        /// freeze authority
        #[arg(long, default_value_t = false)]
        freeze: bool,
    },

    /// Generate project files
    GenerateProject {
        /// Project Name
        #[arg(long, short = 'n')]
        name: Option<String>,

        /// Project Symbol
        #[arg(long, short = 's')]
        symbol: Option<String>,

        /// Project Icon File Path
        #[arg(long, short = 'i')]
        icon: Option<String>,

        /// Description File Path
        #[arg(long, short = 'd')]
        description: Option<String>,

        /// Token Mint
        #[arg(long, default_value_t = 10000000)]
        mint: u64,

        /// Token Decimal
        #[arg(long, default_value_t = 6)]
        decimal: u8,
    },

    /// Generate Wallets for Project
    GenerateWallet {
        /// Wallet generation count
        #[arg(short = 'c', long)]
        count: i32,

        /// Replace current wallets in the project
        #[arg(long, default_value_t = false)]
        replace: bool,
    },

    /// Rug the token
    Rug {
        /// Initial liquidity
        #[arg(short = 'i', long)]
        initial: f64,

        /// Target liquidity before rugging
        #[arg(short = 't', long)]
        target: f64,
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

    /// Check Project Wallet Balance
    Balance {
        /// Balance of the project wallets including payer and WSOL
        #[arg(long, default_value_t = false)]
        all: bool,
    },

    /// Monitor Account Change
    MonitorAccount {
        /// Monitor using websocket if the account changes
        #[arg(short = 'a', long)]
        address: String,
    },

    /// Revoke the token mint authority
    RevokeAuthority {},

    /// Airdrop SPL token to generated wallets
    Airdrop {
        /// percentage amount to distribute to each wallet
        #[arg(short = 'p', default_value_t = 50.0)]
        percentage: f64,

        /// Confirm WSOL Account Confirmation
        #[arg(long, default_value_t = false)]
        confirm: bool,
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
        /// Amount of liquidity to be added in SOL
        #[arg(short = 's', long)]
        amount: f64,

        /// Wait n second before opening the pool
        #[arg(short = 'w',  default_value_t = 15)]
        wait: u64,
    },

    /// Remove liquidity
    RemoveLiquidity {},

    /// Create WSOL Account
    CreateWsol {
        /// Sol to be transferred as WSOL
        #[arg(short = 's', default_value_t = 0.015)]
        amount: f64,

        /// Skip WSOL Account confirmation
        #[arg(long, default_value_t = false)]
        skip_confirm: bool,
    },

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

        /// Buy Overhead (wait time after pool was opened)
        #[arg(short = 'o', default_value_t = 0.0)]
        overhead: f64,
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

    /// Monitor Price Change
    Monitor {
        /// Token Mint
        #[arg(short = 'm')]
        mint: String
    },

    /// Check WSOL Balance
    BalanceWsol {
    },

    /// Auto sell the airdropped token when liquidity pool is added
    AutoSell {
        /// Token Mint
        #[arg(short = 'm', default_value = "So11111111111111111111111111111111111111112")]
        mint: String,

        /// Quote Mint
        #[arg(short = 'q', default_value = "So11111111111111111111111111111111111111112")]
        quote_mint: String,

        /// Sell Overhead (wait time after pool was opened)
        #[arg(short = 'o', default_value_t = 2.0)]
        overhead: f64,

        /// Sell interval
        #[arg(short = 'i', default_value_t = 1.0)]
        interval: f64,

        /// Sell percentage
        #[arg(short = 'p', default_value_t = 100.0)]
        percentage: f64,

        /// Withdraw after selling
        #[arg(long, default_value_t = false)]
        withdraw: bool,

        /// Withdraw destination
        #[arg(long)]
        destination: Option<String>,
    }
}