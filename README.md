# Cliffhanger

Your tokens unlock later. Cliffhanger shows what you can do with them today, and what each choice costs you.

For each vesting grant, the holder has three choices:

- **Sell a slice.** A buyer pays now and is paid first from each unlock until the slice is delivered.
- **Borrow.** A lender advances tokens now and each unlock repays the lender first, fee included.
- **Stake and earn.** The locked escrow earns staking rewards while it waits.

A treasury planner shows every choice under three price paths: price falls, holds and rises. The worst case is always on screen, and every assumption is labelled sourced, demo or entered by you.

Built for the NEAR hackathon (1 to 14 Nov 2026).

## Try it: do X, see Y

Requirements: Python 3.10 or later. For the contract: Rust with the `wasm32-unknown-unknown` target.

1. Run the engine and planner tests.
   ```
   cd poc
   python3 test_loan.py && python3 test_choices.py && python3 test_planner.py
   ```
   You see `OK` three times: 12 loan tests, 10 choice tests, 10 planner tests.

2. Start the local demo.
   ```
   python3 server.py --port 8931
   ```
   Open http://127.0.0.1:8931. You see two demo grants. Borrow, claim, and step time forward with **+30 days**. Each claim pays the lender first.

3. Ask the planner about a position.
   ```
   curl 'http://127.0.0.1:8931/api/plan?locked=365000&price=2&months=12&cliff=3'
   ```
   You see each choice under three price paths. `worst_holder` names the worst case: selling a slice when the price falls.

4. Open the treasury desk.
   Open http://127.0.0.1:8931/desk. You see every position with cash available now by each choice and the worst case in red. Each row opens its month-by-month statement and its three price paths. Add your own position at the bottom.

5. Test the contract.
   ```
   cd ../contract
   cargo test
   ```
   You see 43 tests pass. To build the deployable WASM, see contract/README.md.

6. Open the on-chain page.
   Open `web/index.html` in a browser. It reads the live testnet contract directly: what the contract owes, what it holds, the holder's grants and what they can do today. It also asks NEAR Intents for a live dry-run quote to cash out as USDC on another chain.

## On NEAR testnet

The contract runs on testnet at `cliffhanger-demo-8nov.testnet`, seeded with the same two demo grants. Read them yourself:
```
near contract call-function as-read-only cliffhanger-demo-8nov.testnet get_grants json-args '{"account_id":"alice.cliffhanger-demo-8nov.testnet"}' network-config testnet now
```
You see two grants, 365,000 and 120,000 demo tokens, fully in escrow. Balances are synthetic and testnet tokens have no value.

Build for deploy: `cargo near build non-reproducible-wasm --no-abi` with Rust 1.86 (`RUSTUP_TOOLCHAIN=1.86.0`). cargo-near 0.23 refuses WASM from newer compilers, and the lockfile pins dependencies that still build on 1.86.

### Real tokens, end to end

A second, funded contract, `cliffhanger-vest-8nov.testnet`, holds a real testnet token (CLIFF, from `cliffhanger-token-8nov.testnet`):
- an issuer funds a grant by sending tokens;
- a lender deposits;
- the holder borrows, unlocks (the lender is paid first) and withdraws to their own wallet.

`backing()` always equals the contract's token balance. Check it:
```
near contract call-function as-read-only cliffhanger-vest-8nov.testnet backing json-args '{}' network-config testnet now
near contract call-function as-read-only cliffhanger-token-8nov.testnet ft_balance_of json-args '{"account_id":"cliffhanger-vest-8nov.testnet"}' network-config testnet now
```
You see the same number twice. Every transaction is listed in TESTNET-E2E.md.

## Where the numbers come from

| Assumption | Value | Label | Source |
|---|---|---|---|
| Discount to sell locked tokens early | 40% up to 12 months, 50% for 13 to 24, 60% from 36 | sourced | OFFX median, in The Block, 5 Apr 2026 |
| Price fall at unlock | 25% | sourced | Keyrock, team unlocks across 16,000+ unlocks. Used as the end price, which is our modelling choice |
| Staking yield | 4.5% a year | demo | Secondary reports after NEAR One cut maximum inflation to 2.5%, 21 Oct 2025 |
| Borrow limit and fee | 40% of locked value, 12% a year | demo | Cliffhanger demo values, not a market price |
| Price rise | 25% | demo | A symmetric demo path, not a forecast |

## What is not built

- **NEAR Intents payout.** Intents runs on mainnet only, with no testnet. The on-chain page shows a live dry-run quote from the 1Click API, and nothing is sent.
- **Wallet sign-in.** Actions are signed with the NEAR CLI today. Signing in with a wallet from the page is not built yet.
- **A real staking pool.** The demo contract mints demo rewards. The funded contract refuses to stake, because those rewards would have no tokens behind them.

## Layout

| Folder | What |
|---|---|
| `poc/` | Python engine, planner, local demo server and its tests |
| `contract/` | near-sdk Rust contract, demo and funded modes |
| `token/` | CLIFF, a NEP-141 testnet demo token with no value |
| `web/` | One static page that reads the testnet contract and gets an Intents quote |
| `TESTNET-E2E.md` | The real-token run on testnet, with every transaction ID |

## Disclosure

All Cliffhanger code was written during the hackathon window, starting 8 Nov 2026. No code from before the window is used.

Claude Code and Codex wrote most of the code under human direction.

## License

MIT. See LICENSE.
