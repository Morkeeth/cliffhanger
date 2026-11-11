# Cliffhanger on NEAR testnet: real-token vesting, end to end (8 Nov 2026, ~22:30 to 22:50)

This run proves token vesting end to end with a real token. Testnet only. Every account is a new free faucet account, with keys in the macOS keychain, never printed. Nothing touched mainnet.

## Accounts
| Role | Account |
|---|---|
| Demo token CLIFF (NEP-141, 0 decimals, no value) | `cliffhanger-token-8nov.testnet` |
| Funded vesting contract (`new_funded`) | `cliffhanger-vest-8nov.testnet` |
| Issuer / treasury (token owner) | `cliffhanger-demo-8nov.testnet` (also hosts the earlier synthetic demo contract) |
| Holder | `cliffhanger-holder-8nov.testnet` |
| Lender | `cliffhanger-lender-8nov.testnet` |

## What happened, with transaction IDs
Explorer: `https://explorer.testnet.near.org/transactions/<id>`.

| Step | Tx |
|---|---|
| Deploy token, 10,000,000 CLIFF to issuer | AUQEhhtP1fXPnAncG3FRSrmw4Eg2njxF3UsbZ43GRp4i |
| Deploy funded vesting contract | FodJzhEmqR5aHps2dD2ZwRZQn81PiyWmRBjHSQvrPsoZ |
| Register token storage for contract, holder, lender | 37pAgT8J..., 9F5vKcPW..., 2JUqtdot... |
| Issuer sends lender 1,000,000 CLIFF | 9GsE9AsEKLfXMo4tbEggAZqab8jhYNz46dKxv4KBvePc |
| Issuer funds grant g1 with 365,000 CLIFF via `ft_transfer_call` (started 200 days ago, cliff 30, 365 days) | 8TVKCRiF8UE1Wp6CfkBreHWWX9Eg6EKWBAZ61TrEg8Ye |
| Issuer funds grant g2 with 120,000 CLIFF (cancellable, starts today) | 7jtNGFVGfrF7aUY11MXYcE3foDnp9Q58R9KjahuvzkDC |
| Lender deposits 200,000 CLIFF | EQ1x33qSUYYWjRMnBz7oPssfbsxhnQAcNd23tnAN9oup |
| **Check:** `backing()` = 685,000 and `ft_balance_of(contract)` = 685,000 | view calls |
| Holder borrows 50,000 against g1 (quote: 40% limit, max 66,000) | DUPUu5HnfYdiAgqN1eAh67aMeusGfgvS8Bu1KLDZaPYG |
| Holder unlocks g1: 200,000 vested; lender paid first 52,712 (50,000 + fee 2,712); holder gets the rest | 8f6EsnaSrGAFYiTTgHyjdJErd547hqozsaC9WTLDPir1 |
| **Check:** holder balance in contract 197,288; lender 202,712 | view calls |
| Holder withdraws 197,288 CLIFF to own wallet | tx ID not captured. The first attempt returned a CLI error and did not run (balance unchanged); the retry succeeded, and the token logged `ft_transfer` contract to holder, 197,288, memo "cliffhanger withdraw" |
| **Check:** holder wallet 197,288 CLIFF; holder in contract 0; `backing()` = 487,712 = `ft_balance_of(contract)` | view calls |

The fee figure is from the engine: 50,000 at 12% a year over a 165-day term is 2,712.

## What is real and what is still demo
- **Real on testnet:** the token, the escrow, deposits, grants funded by token transfer, borrowing from a lender's real deposit, the unlock that pays the lender first, and the withdrawal to the holder's wallet. The contract's token balance equals `backing()` after every step.
- **Refused in the funded contract:** stake and collect_rewards. Rewards would be minted with no token behind them, and no staking pool is wired. The synthetic demo contract still shows stake.
- **Sell a slice and repay:** use the same custodial balances, but were not run on chain in this pass.

## Code
- **Contract:** `new_funded`, `ft_on_transfer`, `unlock`, `withdraw` with a callback that restores the balance on failure, and `backing`.
- **Tests:** 43 cargo tests pass (35 earlier and 8 new funded tests). 4 mutants were all caught: withdraw keeps supply, unauthenticated restore, any token can deposit, lender leg skipped.
- **Token:** `token/` (near-contract-standards 5.17.2).
- **Build:** Rust 1.86 with cargo-near 0.23.

## Not done
- **Agent key:** a function-call key on the holder, limited to `unlock, withdraw, borrow, repay, sell_slice` on the vesting contract, so an agent can act and cannot touch the rest of the wallet. Not created yet.
- **Hosted desk:** `web/index.html` reads these accounts from the testnet RPC. A live choice uses the grant's remaining term for the OFFX discount, then a dry NEAR Intents quote. Prepare deposit asks 1Click for one deposit address and a MyNearWallet transfer of NEAR or AURORA. The vesting contract stays on testnet and does not send that deposit. https://morkeeth.github.io/cliffhanger/ updates when this file is merged; this change does not deploy or merge.
