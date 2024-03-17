# SPL Token Creator

## Installation
```bash
$ cargo build --release
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

## Usage

```bash
$ spl-token-creator --help
SPL token management

Usage: spl-token-creator.exe [OPTIONS] <COMMAND>

Commands:
  create            Create a new SPL token
  market            Open an Opendex Market Listing
  airdrop           Airdrop SPL token to generated wallets
  burn              Burn all SPL token
  add-liquidity     Add liquidity
  remove-liquidity  Remove liquidity
  pool-information  Get Market State
  buy               Buy Token
  sell              Sell Token
  project-sell      Sell Project Token
  auto-sell         Auto sell the airdropped token when liquidity pool is added
  help              Print this message or the help of the given subcommand(s)

Options:
  -n, --name <NAME>      Project name
  -c, --config <CONFIG>  Config file [default: config.yaml]
      --dev              Use devnet program ids
      --verbose          verbose log
  -h, --help             Print help
  -V, --version          Print version
```