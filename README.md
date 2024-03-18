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
rpc_url: "https://rpc.ankr.com/solana/mainnet"
wss_url: "wss://rpc.ankr.com/"
wallet_keypair: "bs58"
nft_storage_api_key: "NFTSTORAGE_API_KEY"
project_directory: "/my-project-dir"
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

Usage: spl-token-creator.exe [OPTIONS] <COMMAND>

Commands:
  create            Create a new SPL token
  generate-project  Generate project files
  generate-wallet   Generate Wallets for Project
  market            Open an Opendex Market Listing
  airdrop           Airdrop SPL token to generated wallets
  burn              Burn all SPL token
  add-liquidity     Add liquidity
  remove-liquidity  Remove liquidity
  pool-information  Get Market State
  buy               Buy Token
  sell              Sell Token
  withdraw          Withdraw all SOL from generated accounts
  project-sell      Sell Project Token
  auto-sell         Auto sell the airdropped token when liquidity pool is added
  help              Print this message or the help of the given subcommand(s)

Options:
  -n, --name <NAME>        Project name
  -c, --config <CONFIG>    Config file [default: config.yaml]
      --dev                Use devnet program ids
      --verbose            verbose log
      --keypair <KEYPAIR>  Custom Keypair (base58) file (ex. wallet.yaml)
  -h, --help               Print help
  -V, --version            Print version
```