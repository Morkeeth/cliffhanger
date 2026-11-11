# Mainnet deploy runbook

Oscar runs these. His key signs, and his NEAR pays. The machine prepared and rehearsed every step on testnet (TESTNET-E2E.md, 11 Nov).

## What goes on mainnet
- `cliff.morkeeth.near`: CLIFF token, fixed supply 10,000,000, **no value**. The whole supply goes to `morkeeth.near`. Never sell it or give it out.
- `vest.morkeeth.near`: the hardened vesting escrow, bound to that token.
- `hold.morkeeth.near` and `lend.morkeeth.near`: demo holder and lender.

## Cost
About 9.5 NEAR, mostly storage. Contract code costs 1 NEAR per 100 KB: the token wasm is 184 KB and the vesting wasm is 366 KB. The rest is account and state headroom. Storage NEAR stays locked in the accounts, and deleting an account returns it.

## Steps
Use near-cli-rs 0.31.1 (`npx -y near-cli-rs@0.31.1`, shown as `near`). Keys go to the macOS keychain.

1. Put your key on this Mac (opens MyNearWallet):
   near account import-account using-web-wallet network-config mainnet
2. Create the four accounts from morkeeth.near:
   near account create-account fund-myself cliff.morkeeth.near '2.5 NEAR' autogenerate-new-keypair save-to-keychain sign-as morkeeth.near network-config mainnet sign-with-keychain send
   near account create-account fund-myself vest.morkeeth.near '4.5 NEAR' autogenerate-new-keypair save-to-keychain sign-as morkeeth.near network-config mainnet sign-with-keychain send
   near account create-account fund-myself hold.morkeeth.near '0.5 NEAR' autogenerate-new-keypair save-to-keychain sign-as morkeeth.near network-config mainnet sign-with-keychain send
   near account create-account fund-myself lend.morkeeth.near '0.5 NEAR' autogenerate-new-keypair save-to-keychain sign-as morkeeth.near network-config mainnet sign-with-keychain send
3. Build the wasm (Rust 1.86):
   (cd token && RUSTUP_TOOLCHAIN=1.86.0 RUSTFLAGS="-C link-arg=-s" cargo build --release --target wasm32-unknown-unknown)
   (cd contract && RUSTUP_TOOLCHAIN=1.86.0 RUSTFLAGS="-C link-arg=-s" cargo build --release --target wasm32-unknown-unknown)
4. Deploy and prove, the same script as testnet:
   NET=mainnet TOK=cliff.morkeeth.near VEST=vest.morkeeth.near ISS=morkeeth.near HOLD=hold.morkeeth.near LEND=lend.morkeeth.near scripts/deploy-and-prove.sh
   The script needs the lender to hold CLIFF, so step 4 of the script sends it 1,000,000 from morkeeth.near.

## Known limits, said openly
- The contract is unaudited. A holder of `vest.morkeeth.near`'s full-access key can redeploy and drain it. That is fine only because CLIFF has no value.
- No owner, pause or sweep method. CLIFF sent with a plain `ft_transfer` is stranded.
- Grant cap 32, wallet cap 64.
- NEAR Intents swaps real assets (NEAR, AURORA) at market prices. CLIFF itself is never swapped.
