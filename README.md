# Spectre Discord Wallet Bot

A fun Discord bot for sending Spectre coins on the [Spectre Discord](https://discord.spectre-network.org/) server.

---

## 1. Install Rust

Run the following command to install Rust:

```shell
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
```

## 2. Setup `.env`

Create a file named `.env` and add the following environment variables:

```shell
# Discord Bot Token
DISCORD_TOKEN=

# Specify the network: mainnet, testnet-10, testnet-11, or devnet
SPECTRE_NETWORK=mainnet

# Wallet data path (relative path if using ${data_dir}/*, or specify an absolute path)
WALLET_DATA_PATH=./spectre-wallets

# Optionally, force Spectre Node Address (leave blank or provide a WebSocket RPC address)
FORCE_SPECTRE_NODE_ADDRESS=
```

## 3. Run a Rusty-Spectre Node

For more details, refer to the [rusty-spectre repository](https://github.com/spectre-project/rusty-spectre)

Run the spectre node with the following command:

```shell
./spectred --utxoindex --rpclisten-borsh=default
```

## 4. Run the Bot

```shell
cargo run
```

---

## Commands

- **`/wallet`**: main command for wallet interactions
- **`/create <secret>`**: creates a new wallet
- **`/open <secret>`**: opens your wallet using the secret
- **`/close`**: closes your currently opened wallet
- **`/status`**: check wallet status (opened, initiated, balance).
- **`/destroy`**: permanently deletes your wallet
- **`/restore <mnemonic> <new_secret>`**: restores a wallet from a mnemonic phrase
- **`/export <secret>`**: exports your wallet's mnemonic and xpub
- **`/change_secret <old_secret> <new_secret>`**: lets you change wallet secret

---

- **`/send <user> <amount> <secret>`**: send funds to another user
  - if the recipient doesn’t have a wallet, a transition wallet is created
- **`/claim`**: transfers funds from all transition wallets to your main (owned) wallet
- **`/withdraw <address> <amount> <secret>`**: sends funds to a specified Spectre wallet address
