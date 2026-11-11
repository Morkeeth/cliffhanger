# Cliffhanger

Your tokens unlock later. Cliffhanger shows what you can do with them today, and what each choice costs you.

Built at NEAR HACKATHON (window 19 Oct to 14 Nov 2026). Tracks: Unstoppable DeFi, Real-World Money.

**Live:** https://cliffhanger-five.vercel.app (wallet desk) · https://morkeeth.github.io/cliffhanger/ (read-only viewer). Both read the testnet contract `vest.ch11-lend.testnet`.

For each vesting grant, the holder has three choices:

- **Borrow.** A lender who has offered funds advances tokens now, and each unlock repays the lender first, fee included.
- **Sell a slice.** A buyer who has offered funds pays now and is paid first from each unlock until the slice is delivered.
- **Wait.** Keep every unlock. (Staking shows in the local demo only; the funded contract refuses unbacked rewards.)

A treasury planner shows every choice under three price paths: price falls, holds and rises. The worst case is always on screen, and every assumption is labelled sourced, demo or entered by you.

## How it uses NEAR

| Piece | What is real |
|---|---|
| Escrow contract (Rust, near-sdk) | On testnet. Grants arrive by NEP-141 `ft_transfer_call`. Each unlock pays the lender or buyer first, then credits the holder. `backing()` equals the contract's token balance after every step. |
| Agent payday | The holder can give an agent a function-call key limited to `unlock`. The agent releases vested tokens on schedule. It cannot withdraw: every other money move needs a 1 yoctoNEAR wallet confirmation, which a function-call key cannot attach. Proven on testnet, see TESTNET-E2E.md. |
| NEAR Intents (1Click) | Mainnet. Dry quotes price a choice. Prepare deposit gets one deposit address and opens MyNearWallet with the transfer: send NEAR or AURORA, receive AURORA on NEAR or Ethereum, or USDC on Base. The page never broadcasts; the holder signs. |
| Wallet | Testnet MyNearWallet connect, unlock and withdraw on the escrow; mainnet MyNearWallet for the Intents deposit. |
| CLIFF token | A NEP-141 demo token with no value and fixed supply. |

Before any mainnet deploy, two independent reviews found six ways the escrow could lose or lock funds. All six are fixed and replayed on chain; the transaction ids are in TESTNET-E2E.md. MAINNET.md is the deploy runbook. The escrow is not on mainnet yet.

## Wallet desk locally

Node 22 is required (`nvm use` reads `.nvmrc`):

```
npm ci --ignore-scripts
npm test
npm run build
npm run dev
```

Open http://127.0.0.1:8932, click **Connect testnet wallet**, then **Read my account**. Unlock needs no deposit. Withdraw asks MyNearWallet to confirm 1 yoctoNEAR. A fresh wallet sees no grants until an issuer funds one for it (`scripts/deploy-and-prove.sh` shows how). No operation is retried automatically.

## Try it: do X, see Y

Requirements: Python 3.10 or later. For the contract: Rust with the `wasm32-unknown-unknown` target.

1. Run the engine and planner tests.
   ```
   cd poc
   python3 test_loan.py && python3 test_choices.py && python3 test_planner.py && python3 test_intents.py
   ```
   You see `OK` four times: 12 loan tests, 10 choice tests, 10 planner tests, 11 intent tests. The intent tests build the quote and the wallet transaction. They do not call the network and they do not sign.

2. Start the local demo.
   ```
   python3 server.py --port 8931
   ```
   Open http://127.0.0.1:8931. You see five CLIFF grants: one still in its cliff, one not yet vesting, one already vesting, one fully vested, and one the issuer can cancel. Borrow, sell a slice, stake, claim, and step time forward with **+30 days**. Each claim pays the lender or the buyer first. The 24-month hire sells at 50% off; a grant with a year or less left sells at 40% off. Below those cards are three AURORA grants, two already vesting and one fully vested. They are not in the CLIFF ledger. **Dry quote** prices the choice. **Prepare deposit** asks NEAR Intents for one deposit address. Enter your mainnet NEAR account first. A CLIFF choice sends NEAR. An AURORA choice sends AURORA. The link opens MyNearWallet with the transfer ready to sign. This page does not sign it.

3. Ask the planner about a position.
   ```
   curl 'http://127.0.0.1:8931/api/plan?locked=365000&price=2&months=12&cliff=3'
   ```
   You see each choice under three price paths. `worst_holder` names the worst case: selling a slice when the price falls.

4. Open the treasury desk.
   Open http://127.0.0.1:8931/desk. You see every still-locked position, including two AURORA grants that have already started vesting. The row shows how much is still locked and how much has vested. Cash available now is listed by each choice, and the worst case is in red. Fully vested grants, the finished CLIFF grant and the finished AURORA grant, are on the action page, because nothing remains locked to plan. Each row opens its month-by-month statement and its three price paths. Add your own position at the bottom.

5. Test the contract.
   ```
   cd ../contract
   cargo test
   ```
   You see 53 tests pass. To build the deployable WASM, see contract/README.md.

6. Open the on-chain page.
   Run `npm run dev` from the repository root and open http://127.0.0.1:8932 (opening the file directly does not load the wallet modules). It reads the live testnet contract directly: what the contract owes, what it holds, the holder's grants and what they can do today. Choose a live grant or a local row, then a choice. A dry quote prices three routes. Prepare deposit, on one route, asks 1Click for a deposit address and offers a MyNearWallet transfer. A CLIFF choice sends NEAR. A local AURORA row sends AURORA. The sale uses the OFFX discount for the months still left (`term_days` on a live grant). The table under the quote is the local book, not extra tokens on the contract.

## On NEAR testnet

The contract runs on testnet at `cliffhanger-demo-8nov.testnet`, seeded with the same two demo grants. Read them yourself:
```
near contract call-function as-read-only cliffhanger-demo-8nov.testnet get_grants json-args '{"account_id":"alice.cliffhanger-demo-8nov.testnet"}' network-config testnet now
```
You see two grants, 365,000 and 120,000 demo tokens, fully in escrow. Balances are synthetic and testnet tokens have no value.

Build for deploy: `cargo near build non-reproducible-wasm --no-abi` with Rust 1.86 (`RUSTUP_TOOLCHAIN=1.86.0`). cargo-near 0.23 refuses WASM from newer compilers, and the lockfile pins dependencies that still build on 1.86.

### Real tokens, end to end

The live funded contract is `vest.ch11-lend.testnet`, holding CLIFF from `tok.ch11-lend.testnet`. The first run, before the hardening, used `cliffhanger-vest-8nov.testnet`; both are in TESTNET-E2E.md. In the live contract:
- an issuer funds a grant by sending tokens;
- a lender deposits;
- the holder borrows, unlocks (the lender is paid first) and withdraws to their own wallet.

`backing()` always equals the contract's token balance. Check it:
```
near contract call-function as-read-only vest.ch11-lend.testnet backing json-args '{}' network-config testnet now
near contract call-function as-read-only tok.ch11-lend.testnet ft_balance_of json-args '{"account_id":"vest.ch11-lend.testnet"}' network-config testnet now
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

- **NEAR Intents payout.** Intents runs on mainnet only, with no testnet. The demo and the on-chain page price a choice with a dry quote, then Prepare deposit asks 1Click for one deposit address. A CLIFF choice is funded with NEAR. A vested AURORA grant is funded with AURORA. The holder signs that transfer in MyNearWallet: the page reads the account's full-access key and a fresh block, then opens the wallet with the unsigned transaction. Aurora's own chain is not in the live token list, so the page does not invent a route there. Stake is not a swap: it pays demo rewards. The page does not broadcast the transfer. The vesting contract is not on mainnet: the escrow is unaudited, and it is not the account that pays Intents.
- **Browser wallet journey.** The desk is live with testnet MyNearWallet connect, unlock and withdrawal. Automated controller checks pass; a real browser wallet signature and receipt are not yet recorded. Borrow and sell are signed with the NEAR CLI, as in the proof script.
- **A real staking pool.** The demo contract mints demo rewards. The funded contract refuses to stake, because those rewards would have no tokens behind them.

## Layout

| Folder | What |
|---|---|
| `poc/` | Python engine, planner, local demo server and its tests |
| `contract/` | near-sdk Rust contract, demo and funded modes |
| `token/` | CLIFF, a NEP-141 demo token with no value |
| `scripts/` | `deploy-and-prove.sh`: deploy and init in one transaction, then prove the journey and the refusals |
| `MAINNET.md` | Mainnet deploy runbook; the owner signs every step |
| `web/` | Vite entry with testnet reads, MyNearWallet unlock/withdraw and Intents dry quotes |
| `docs/` | Existing read-only GitHub Pages viewer; kept separate from the wallet build |
| `TESTNET-E2E.md` | The real-token run on testnet, with every transaction ID |

## Disclosure

All Cliffhanger code was written inside the hackathon window (19 Oct to 14 Nov 2026); the first commit is 9 Nov 2026. No code from before the window is used.

Coding agents (Claude Code, Codex and Cursor agents) wrote most of the code under human direction. Independent agent reviews checked the contract before each merge.

## Libraries and services

near-sdk and near-contract-standards 5.17.2 (Rust), @near-wallet-selector 10.1.4 with MyNearWallet, Vite 6 with vite-plugin-node-polyfills and util, NEAR Intents 1Click API, NEAR public RPC, near-cli-rs 0.31.1 for deploy scripts.

## License

MIT. See LICENSE.

## Hosting

Use the repository root, Node **22.x**, install command **npm ci**, build command **npm run build**, and output directory **dist**. `vercel.json` supplies the Vite build settings; `.nvmrc` and package engines pin the Node major. No environment variables, server, issuer keys, or wallet keys are needed. Build and preview locally with `npm run build` then `npm run preview` (http://127.0.0.1:8933).

The deployable entry is `web/index.html`, bundled into `dist/index.html` with relative hashed assets. Deploy **dist**, not `docs` or the repository files. `docs/index.html` deliberately remains the separate read-only GitHub Pages entry. There are no client-side path routes, so no catch-all rewrite is required. Preserve query strings on `/`: MyNearWallet returns account selection and transaction results there. The installed connector derives its callback URL from the current page URL. Use one stable HTTPS origin for the full connect/sign/return test; localhost, preview URLs and production have separate browser storage. Do not change origins midway through a wallet flow or assume account selection carries between them.

The browser calls NEAR testnet RPC, testnet MyNearWallet and the live 1Click dry-quote API directly. Google Fonts is an optional external visual dependency. A restrictive CSP must permit those connections and the bundled module. Hosted CORS, wallet return and final RPC receipts must be checked on the chosen origin; a successful static build does not prove them. No additional backend or key-bearing proxy is included.

Vercel supports Vite and Node 22; see [Vite on Vercel](https://vercel.com/docs/frameworks/frontend/vite) and [supported Node versions](https://vercel.com/docs/functions/runtimes/node-js/node-js-versions). It is deployed at https://cliffhanger-five.vercel.app.
