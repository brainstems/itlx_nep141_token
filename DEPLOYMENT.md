# ITLX Token Deployment Guide

This guide documents the step-by-step process for deploying the ITLX token on NEAR testnet.

## Prerequisites

- NEAR CLI RS installed (new Rust-based CLI)
- Rust and Cargo installed
- `wasm-opt` installed for WebAssembly optimization
- A NEAR testnet account with sufficient balance
- Ledger device (optional, for secure deployment)

## Create a new NEAR testnet account

```bash
near account create-account sponsor-by-faucet-service intellex_contract_owner.testnet use-ledger --seed-phrase-hd-path 'm/44'\''/397'\''/0'\''/0'\''/1'\''' network-config testnet create

## Step 1: Build the Contract

Build the contract with optimizations:

```bash
RUSTFLAGS='-C link-arg=-s' cargo build --target wasm32-unknown-unknown --release
```

## Step 2: Optimize the WASM File

Optimize the compiled WASM file for smaller size and better performance:

```bash
wasm-opt -Oz -o target/wasm32-unknown-unknown/release/fungible_token.wasm target/wasm32-unknown-unknown/release/fungible_token.wasm
```

## Step 3: Prepare Metadata

1. Create a `metadata.json` file with extended token information:
```json
{
    "name": "Intellex AI Protocol Token",
    "symbol": "ITLX",
    "decimals": 24,
    "totalSupply": "1000000000",
    "website": "https://intellex.xyz",
    "description": "Intellex AI Protocol - Decentralized AI Infrastructure",
    "socials": {
        "twitter": "https://twitter.com/intellex_ai",
        "telegram": "https://t.me/intellex_ai",
        "github": "https://github.com/intellex-ai",
        "medium": "https://medium.com/@intellex"
    },
    "links": {
        "website": "https://intellex.xyz",
        "whitepaper": "https://intellex.xyz/whitepaper",
        "documentation": "https://docs.intellex.xyz"
    },
    "icon": "data:image/svg+xml,...", // Your SVG icon here
    "spec": "ft-1.0.0",
    "reference": "https://raw.githubusercontent.com/brainstems/itlx_nep141_token/refs/heads/master/metadata.json"
}
```

2. Calculate the metadata hash:
```bash
cat metadata.json | shasum -a 256 | cut -d ' ' -f 1 | xxd -r -p | base64
```

3. Commit and push metadata.json to the repository:
```bash
git add metadata.json
git commit -m "Update metadata.json"
git push
```

## Step 4: Deploy and Initialize the Contract

### Using Ledger (Recommended)

Deploy and initialize the contract using your Ledger device:

```bash
near contract deploy intellex_contract_owner.testnet use-file ./target/wasm32-unknown-unknown/release/fungible_token.wasm with-init-call new_default_meta json-args '{"owner_id": "intellex_contract_owner.testnet", "total_supply": "1000000000000000000000000000000000"}' prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' network-config testnet sign-with-ledger --seed-phrase-hd-path 'm/44'\''/397'\''/0'\''/0'\''/1'\''' send
```

Replace `ACCOUNT_ID` with your account (e.g., `intellex_contract_owner.testnet`).

When prompted, enter your Ledger HD path (default is usually `m/44'/397'/0'/0'/1'`).

### Using Access Keys (Alternative)

If not using a Ledger, you can deploy using your access keys:

```bash
near contract deploy ACCOUNT_ID use-file ./target/wasm32-unknown-unknown/release/fungible_token.wasm \
  with-init-call new_default_meta \
  json-args '{"owner_id": "ACCOUNT_ID", "total_supply": "1000000000000000000000000000000000"}' \
  prepaid-gas '100 TeraGas' \
  attached-deposit '0 NEAR' \
  network-config testnet \
  sign-with-access-key-file \
  send
```

## Step 5: Verify Deployment

1. Check the total supply:
```bash
near contract call-function as-read-only ACCOUNT_ID ft_total_supply json-args '{}' network-config testnet
```

2. Verify metadata:
```bash
near contract call-function as-read-only ACCOUNT_ID ft_metadata json-args '{}' network-config testnet
```

3. Check owner's balance:
```bash
near contract call-function as-read-only ACCOUNT_ID ft_balance_of json-args '{"account_id": "ACCOUNT_ID"}' network-config testnet
```

## Important Notes

1. The total supply is set to 1 billion ITLX tokens with 24 decimals:
   - Base amount: 1,000,000,000 (1 billion)
   - With 24 decimals: 1,000,000,000 * 10^24 = 1,000,000,000,000,000,000,000,000,000,000,000
2. The metadata reference points to the GitHub repository's raw file URL
3. The reference_hash ensures the integrity of the external metadata
4. The contract owner has full initial supply of tokens
5. Storage deposits are required for new accounts before they can receive tokens

## Post-Deployment Tasks

1. Verify the metadata.json file is accessible at the reference URL
2. Verify the metadata hash matches the one in the contract
3. Test token transfers and other functionality
4. Register the token on NEAR explorers and listing platforms

## Common Issues and Solutions

1. If the social profiles are not showing up in the explorer:
   - Verify the metadata.json file is accessible at the reference URL
   - Ensure the reference URL in the contract matches the actual file location
   - Check that the reference_hash matches the current metadata.json content
   - If needed, redeploy the contract with updated reference and hash

2. If the total supply appears incorrect:
   - Remember to include all 24 decimal places in the total_supply parameter
   - The display amount will be 1,000,000,000 but the actual parameter needs all zeros

3. If using Ledger:
   - Make sure your Ledger device is connected and unlocked
   - The NEAR app is open on your Ledger
   - The correct HD path is used (usually `m/44'/397'/0'/0'/1'`) 