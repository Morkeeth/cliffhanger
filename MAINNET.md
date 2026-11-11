# Mainnet deploy runbook

Oscar runs these. His key signs, and his NEAR pays. Every step marked **OSCAR CLICK** signs with his key or spends his NEAR. The machine prepared and rehearsed every step on testnet (TESTNET-E2E.md, 11 and 12 Nov).

Not required to enter: the event accepts testnet-only projects (HACKATHON.md Q-010). Mainnet makes the demo stronger.

## What goes on mainnet
- `cliff.morkeeth.near`: CLIFF token, fixed supply 10,000,000, **no value**. The whole supply goes to `morkeeth.near`. Never sell it or give it out.
- `vest.morkeeth.near`: the hardened vesting escrow, bound to that token.
- `hold.morkeeth.near` and `lend.morkeeth.near`: demo holder and lender.

## Cost
About 9.5 NEAR, mostly storage. Contract code costs 1 NEAR per 100 KB: the token wasm is 184 KB and the vesting wasm is 366 KB. The rest is account and state headroom. Storage NEAR stays locked in the accounts, and deleting an account returns it.

## Steps
Use near-cli-rs 0.31.1 (`npx -y near-cli-rs@0.31.1`, shown as `near`). Keys go to the macOS keychain.

1. **OSCAR CLICK.** Put your key on this Mac (opens MyNearWallet; approve there):
   near account import-account using-web-wallet network-config mainnet
2. **OSCAR CLICK, spends about 8.5 NEAR.** Create the four accounts from morkeeth.near:
   near account create-account fund-myself cliff.morkeeth.near '2.5 NEAR' autogenerate-new-keypair save-to-keychain sign-as morkeeth.near network-config mainnet sign-with-keychain send
   near account create-account fund-myself vest.morkeeth.near '4.5 NEAR' autogenerate-new-keypair save-to-keychain sign-as morkeeth.near network-config mainnet sign-with-keychain send
   near account create-account fund-myself hold.morkeeth.near '0.5 NEAR' autogenerate-new-keypair save-to-keychain sign-as morkeeth.near network-config mainnet sign-with-keychain send
   near account create-account fund-myself lend.morkeeth.near '0.5 NEAR' autogenerate-new-keypair save-to-keychain sign-as morkeeth.near network-config mainnet sign-with-keychain send
3. No signing. Build the wasm (Rust 1.86):
   (cd token && RUSTUP_TOOLCHAIN=1.86.0 RUSTFLAGS="-C link-arg=-s" cargo build --release --target wasm32-unknown-unknown)
   (cd contract && RUSTUP_TOOLCHAIN=1.86.0 RUSTFLAGS="-C link-arg=-s" cargo build --release --target wasm32-unknown-unknown)
4. **OSCAR CLICK, signs about 15 transactions with the keys from steps 1 and 2.** Deploy and prove, the same script as testnet:
   NET=mainnet TOK=cliff.morkeeth.near VEST=vest.morkeeth.near ISS=morkeeth.near HOLD=hold.morkeeth.near LEND=lend.morkeeth.near scripts/deploy-and-prove.sh
   The script needs the lender to hold CLIFF, so step 4 of the script sends it 1,000,000 from morkeeth.near.
5. **Optional, OSCAR CLICK.** Agent payday on mainnet: add `AGENT_KEY_FILE=/path/agent.json` to step 4. The script adds a key to `hold.morkeeth.near` that can only call `unlock` on `vest.morkeeth.near`, with a 0.25 NEAR gas allowance, runs the unlock as the agent and checks that the same key cannot withdraw. Remove that key afterwards with `near account delete-keys hold.morkeeth.near public-keys <agent public key> network-config mainnet sign-with-keychain send`.
6. No signing. Point the site at mainnet: change the contract names in `web/index.html`, `docs/index.html` and `web/wallet-policy.js`, and the wallet network from testnet to mainnet, then redeploy. This step is a separate code change with its own review.

## What to say in the demo

CLIFF proves the vesting and the repayment order. The Intents payout uses your real NEAR or AURORA. CLIFF itself is never swapped.

## Known limits, said openly
- The contract is unaudited. A holder of `vest.morkeeth.near`'s full-access key can redeploy and drain it. That is fine only because CLIFF has no value.
- No owner, pause or sweep method. CLIFF sent with a plain `ft_transfer` is stranded.
- Grant cap 32, wallet cap 64.
- NEAR Intents swaps real assets (NEAR, AURORA) at market prices. CLIFF itself is never swapped.
