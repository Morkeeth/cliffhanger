# Cliffhanger vesting contract (near-sdk 5.17.2)

One contract, two ways to start it:

- `new_demo(demo_balances)` seeds synthetic balances for a demo. Transfers stay disabled. Deployed at `cliffhanger-demo-8nov.testnet`.
- `new_funded(token_contract)` holds a real NEP-141 token. Deployed at `cliffhanger-vest-8nov.testnet`, with the CLIFF token at `cliffhanger-token-8nov.testnet`.

## Funded mode
- **Tokens in.** They enter only through `ft_on_transfer` from the configured token.
  - `msg` `{"action":"deposit"}` credits the sender's balance.
  - `msg` `{"action":"create_grant", ...}` funds a new grant for a holder with exactly the tokens sent.
  - Any failure panics, and the token contract refunds the full amount.
- **Borrow.** `borrow(grant_id, amount, lender)` moves the lender's deposited tokens to the holder. The limit is 40% of unvested value, and the fee is 12% a year (demo values).
- **Sell a slice.** `sell_slice(grant_id, slice, price, buyer)` sells part of future unlocks for tokens now.
- **Unlock.** `unlock(grant_id)` settles what has vested. The lender or buyer is paid first, and the holder gets the rest.
- **Withdraw.** `withdraw(amount)` sends real tokens to the caller's wallet. If the transfer fails, a callback restores the balance.
- **Backing.** `backing()` returns the tokens the contract owes: balances plus escrow plus in-flight amounts. It always equals the contract's `ft_balance_of`.
- **Stake.** `stake` and `collect_rewards` are refused here, because rewards would have no tokens behind them. The demo initializer still shows the stake choice.

## Views
`get_grants(account_id)`, `options(account_id)`, `quote(grant_id, amount)`, `demo_balance(account_id)`, `backing()`, `accounting_mode()`.

## Build and test
```
cargo test
RUSTUP_TOOLCHAIN=1.86.0 cargo near build non-reproducible-wasm --no-abi
```
cargo test runs 43 tests: the engine, the three choices, the token-transfer legs and callbacks, and funded mode.

The deploy build uses Rust 1.86, because cargo-near 0.23 refuses WASM from newer compilers. The lockfile pins dependencies that still build on 1.86.

## Limits
- Testnet only. No audit.
- No staking pool is wired.
- Gas bounds are prototype values, not measured costs: claims need 140 Tgas, and withdraw needs 60 Tgas.
- If a token-transfer callback hits an engine error, the grant is marked `NeedsReconcile` and blocked. Nothing is resent automatically.
