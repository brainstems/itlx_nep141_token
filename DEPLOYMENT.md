# ITLX Token Deployment Guide

This guide documents the step-by-step process for deploying the ITLX token on NEAR testnet.

## Prerequisites

- NEAR CLI installed
- Rust and Cargo installed
- `wasm-opt` installed for WebAssembly optimization
- A NEAR testnet account with sufficient balance

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

## Step 4: Deploy the Contract

1. Delete existing contract state if needed:
```bash
near delete itlx-token.intellex_protocol_activators_1.testnet intellex_protocol_activators_1.testnet
```

2. Create a new account for the contract:
```bash
near create-account itlx-token.intellex_protocol_activators_1.testnet --masterAccount intellex_protocol_activators_1.testnet --initialBalance 5
```

3. Deploy the contract:
```bash
near deploy itlx-token.intellex_protocol_activators_1.testnet target/wasm32-unknown-unknown/release/fungible_token.wasm --force
```

## Step 5: Initialize the Contract

Initialize the contract with the prepared metadata (replace YOUR_CALCULATED_HASH with the actual hash from step 3.2):
```bash
near call itlx-token.intellex_protocol_activators_1.testnet new '{
    "owner_id": "intellex_protocol_activators_1.testnet",
    "total_supply": "1000000000000000000000000000000000",
    "metadata": {
        "spec": "ft-1.0.0",
        "name": "Intellex AI Protocol Token",
        "symbol": "ITLX",
        "icon": "YOUR_ICON_HERE",
        "reference": "https://raw.githubusercontent.com/brainstems/itlx_nep141_token/refs/heads/master/metadata.json",
        "reference_hash": "YOUR_CALCULATED_HASH",
        "decimals": 24
    }
}' --accountId intellex_protocol_activators_1.testnet
```

## Step 6: Verify Deployment

1. Check the total supply:
```bash
near view itlx-token.intellex_protocol_activators_1.testnet ft_total_supply
```

2. Verify metadata:
```bash
near view itlx-token.intellex_protocol_activators_1.testnet ft_metadata
```

3. Check owner's balance:
```bash
near view itlx-token.intellex_protocol_activators_1.testnet ft_balance_of '{"account_id": "intellex_protocol_activators_1.testnet"}'
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