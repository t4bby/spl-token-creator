# SPL Token Creator

## Installation
### Linux
```bash
$ cargo build --release
$ cd target/release
$ ./spl-token-creator --help
```
### Windows
```bash
$ cargo build --release
$ cd target/release
$ spl-token-creator.exe --help
```

## Configuration
config.yaml
```yaml
mainnet_transaction_http_endpoint: ''
mainnet_liquidity_wss_endpoint: ''
mainnet_pool_wss_endpoint: ''
devnet_transaction_http_endpoint: ''
devnet_liquidity_wss_endpoint: ''
devnet_pool_wss_endpoint: ''
nft_storage_api_key: ''
project_directory: ''
```

## Project Configuration
project-dir/project-name/config.yaml
```yaml
name: NAME
symbol: SYMBOL
description: ''
mint_amount: 10000000
decimal: 6
image_filename: icon.jpg
metadata_uri: ''
token_keypair: ''
telegram: ''
tags: []
wallets: []
wsol_wallets: []
```

## Keypair Configuration
wallet.yaml
```yaml 
key: "your bs58 key here"
```

## Usage

```bash
$ spl-token-creator --help
SPL token management

Usage: spl-token-creator.exe [OPTIONS] --keypair <KEYPAIR> <COMMAND>

Commands:
  create            Create a new SPL token
  generate-project  Generate project files
  generate-wallet   Generate Wallets for Project
  market            Open an Opendex Market Listing
  balance           Check Project Wallet Balance
  monitor-account   Monitor Account Change
  revoke-authority  Revoke the token mint authority
  airdrop           Airdrop SPL token to generated wallets
  burn              Burn all SPL token
  add-liquidity     Add liquidity
  remove-liquidity  Remove liquidity
  create-wsol       Create WSOL Account
  pool-information  Get Market State
  buy               Buy Token
  sell              Sell Token
  withdraw          Withdraw all SOL from generated accounts
  project-sell      Sell Project Token
  monitor           Monitor Price Change
  balance-wsol      Check WSOL Balance
  auto-sell         Auto sell the airdropped token when liquidity pool is added
  help              Print this message or the help of the given subcommand(s)

Options:
  -n, --name <NAME>        Project name
  -c, --config <CONFIG>    Config file [default: config.yaml]
      --dev                Use devnet program ids
      --verbose            verbose log
  -k, --keypair <KEYPAIR>  Custom Keypair (base58) file (ex. wallet.yaml)
  -h, --help               Print help
  -V, --version            Print version
```