# Cliffhanger

Your tokens unlock later. Cliffhanger shows what you can do with them today, and what each choice costs you.

For each vesting grant, the holder has three choices:

- **Sell a slice.** A buyer pays now and is paid first from each unlock until the slice is delivered.
- **Borrow.** A lender advances tokens now and each unlock repays the lender first, fee included.
- **Stake and earn.** The locked escrow earns staking rewards while it waits.

A treasury planner shows every choice under three price paths: price falls, holds and rises. The worst case is always on screen, and every assumption is labelled sourced, demo or entered by you.

Built for the NEAR hackathon (1 to 14 Nov 2026).

## Open the live testnet viewer

https://morkeeth.github.io/cliffhanger/

This page reads the funded contract from NEAR testnet. It does not connect a wallet or sign transactions. Choose a holder and load grants; see contract backing, actual token balance and available actions. The Intents quote is a mainnet dry run, with no transfer.

## Local wallet journey (prepared 11 November; not published)

Node 22 is required (`nvm use` reads `.nvmrc`). From the repository root:

```
npm ci --ignore-scripts
npm test
npm run build
npm run dev
```

Open http://127.0.0.1:8932. The local desk adds testnet MyNearWallet account selection, a separate review and confirmation for each unlock or withdrawal, and final receipt checks. Connect does not request a function-call access key. The batch signing path asks MyNearWallet to approve every transaction, including if the browser already has an old access key. Package versions and dependency integrity are pinned in package-lock.json. `dist/` is the build to host when publication is authorized; the existing public viewer remains unchanged.

Human wallet steps still required:

1. Click **Connect testnet wallet**, and select a testnet account you control in MyNearWallet. No account is created or funded by this page.
2. Click **Read my account**. An account without a funded grant sees no unlocks. The existing funded holder is `cliffhanger-holder-8nov.testnet`; its key has not been imported into a browser by this work. A fresh wallet requires an issuer-funded grant and token storage registration before a real withdrawal can be exercised.
3. For an available grant, click **Unlock g1**, review the preview, then **Confirm testnet transaction** and approve in MyNearWallet. Unlock pays a lender/buyer first and credits the rest inside Cliffhanger.
4. Wait for refreshed balances. Enter a whole CLIFF amount no larger than the available balance, click **Review withdrawal**, then **Confirm testnet transaction** and approve in MyNearWallet. Check the final receipt and wallet balance. A failed token transfer restores the contract balance and is not reported as a completed withdrawal.
5. Disconnect when finished. No operation is retried automatically.

The automated controller exercise uses a controlled in-memory wallet, DOM and RPC response. It checks account binding, separate review/confirmation, gas, exact amounts, callback failure, account switching, and no automatic retry. It is **not** proof that a real browser wallet or a new on-chain transaction completed. No wallet/key/permission was created, no transaction was signed, and nothing was published in this pass. A reload in the existing Chrome window confirmed the enabled connect button and live backing/grant reads after fixing browser SDK polyfills. A human-approved real wallet run remains required. Browser borrow/repay/sell controls, issuer onboarding, restricted agent signing, real staking and executed Intents payout remain outside this slice.

The treasury planner below runs locally and remains distinct from this on-chain viewer.

## Try it: do X, see Y

Requirements: Python 3.10 or later. For the contract: Rust with the `wasm32-unknown-unknown` target.

1. Run the engine and planner tests.
   ```
   cd poc
   python3 test_loan.py && python3 test_choices.py && python3 test_planner.py && python3 test_intents.py
   ```
   You see `OK` four times: 12 loan tests, 10 choice tests, 10 planner tests, 8 intent tests. The intent tests build dry quote requests and do not call the network.

2. Start the local demo.
   ```
   python3 server.py --port 8931
   ```
   Open http://127.0.0.1:8931. You see five CLIFF grants: one still in its cliff, one not yet vesting, one already vesting, one fully vested, and one the issuer can cancel. Borrow, sell a slice, stake, claim, and step time forward with **+30 days**. Each claim pays the lender or the buyer first. The 24-month hire sells at 50% off; a grant with a year or less left sells at 40% off. Below those cards are three AURORA grants, two already vesting and one fully vested. They are not in the CLIFF ledger. **Dry quote** on a card asks NEAR Intents for that choice. A CLIFF quote spends NEAR at the price you enter. An AURORA quote spends the AURORA token.

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
   You see 43 tests pass. To build the deployable WASM, see contract/README.md.

6. Open the on-chain page.
   Open `web/index.html` in a browser. It reads the live testnet contract directly: what the contract owes, what it holds, the holder's grants and what they can do today. Choose a live grant or a local row, then a choice. A CLIFF choice is priced in NEAR at the figure you enter. The sale uses the OFFX discount for the months still left (`term_days` on a live grant). The page asks NEAR Intents for three dry quotes: AURORA on NEAR, AURORA on Ethereum, and USDC on Base. A local AURORA row is quoted by sending that AURORA token to wNEAR, to AURORA on Ethereum, and to USDC on Base. The table under the quote is the local book, not extra tokens on the contract. https://morkeeth.github.io/cliffhanger/ serves this file only after it is merged. This change does not merge or deploy.

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

- **NEAR Intents payout.** Intents runs on mainnet only, with no testnet. The demo and the on-chain page ask the 1Click API for dry quotes sized to a real choice. CLIFF choices can arrive as the AURORA token on NEAR or on Ethereum, or as USDC on Base. A vested AURORA grant is quoted by sending the AURORA token. Aurora's own chain is not in the live token list, so the page does not invent a route there. Stake is not quoted: it pays demo rewards, not cash Intents can swap. Nothing is sent. The vesting contract is not on mainnet: the escrow is unaudited, testnet is enough for this hackathon, and the only mainnet call is that dry quote.
- **Published wallet journey.** The release candidate adds testnet MyNearWallet connect, unlock and withdrawal. It has automated controller checks; a real browser wallet signature and final token receipt are still unproven. The existing GitHub Pages viewer stays read-only.
- **A real staking pool.** The demo contract mints demo rewards. The funded contract refuses to stake, because those rewards would have no tokens behind them.

## Layout

| Folder | What |
|---|---|
| `poc/` | Python engine, planner, local demo server and its tests |
| `contract/` | near-sdk Rust contract, demo and funded modes |
| `token/` | CLIFF, a NEP-141 testnet demo token with no value |
| `web/` | Vite entry with testnet reads, MyNearWallet unlock/withdraw and Intents dry quotes |
| `docs/` | Existing read-only GitHub Pages viewer; kept separate from the wallet build |
| `TESTNET-E2E.md` | The real-token run on testnet, with every transaction ID |

## Disclosure

All Cliffhanger code was written during the hackathon window, starting 8 Nov 2026. No code from before the window is used.

Claude Code and Codex wrote most of the code under human direction.

## License

MIT. See LICENSE.

## Hosting the release candidate

Use the repository root, Node **22.x**, install command **npm ci**, build command **npm run build**, and output directory **dist**. `vercel.json` supplies the Vite build settings; `.nvmrc` and package engines pin the Node major. No environment variables, server, issuer keys, or wallet keys are needed. Build and preview locally with `npm run build` then `npm run preview` (http://127.0.0.1:8933).

The deployable entry is `web/index.html`, bundled into `dist/index.html` with relative hashed assets. Deploy **dist**, not `docs` or the repository files. `docs/index.html` deliberately remains the separate read-only GitHub Pages entry. There are no client-side path routes, so no catch-all rewrite is required. Preserve query strings on `/`: MyNearWallet returns account selection and transaction results there. The installed connector derives its callback URL from the current page URL. Use one stable HTTPS origin for the full connect/sign/return test; localhost, preview URLs and production have separate browser storage. Do not change origins midway through a wallet flow or assume account selection carries between them.

The browser calls NEAR testnet RPC, testnet MyNearWallet and the live 1Click dry-quote API directly. Google Fonts is an optional external visual dependency. A restrictive CSP must permit those connections and the bundled module. Hosted CORS, wallet return and final RPC receipts must be checked on the chosen origin; a successful static build does not prove them. No additional backend or key-bearing proxy is included.

Vercel supports Vite and Node 22; see [Vite on Vercel](https://vercel.com/docs/frameworks/frontend/vite) and [supported Node versions](https://vercel.com/docs/functions/runtimes/node-js/node-js-versions). Hosting is prepared but this candidate has not been deployed.
