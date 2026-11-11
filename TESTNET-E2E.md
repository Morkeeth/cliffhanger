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

# Hardened contract on testnet (11 Nov 2026)

An independent review found three ways to lose funds and one init race. A second review (Codex) found a stranded-withdrawal case and an unguarded unlock. All six are fixed, and this run proves the fixes on chain. New faucet accounts, keys in the macOS keychain. Reproduce with `scripts/deploy-and-prove.sh`.

## What changed in the contract
- **Consent.** A holder can borrow or sell a slice only from an account that offered. Lenders and buyers deposit with `{"action":"offer"}` or call `set_lendable`. Before, a holder could name any account and take its balance.
- **Wallet confirmation.** `borrow`, `sell_slice`, `repay`, `cancel`, `unlock`, `withdraw`, `create_grant`, `unstake` and `set_lendable` need exactly 1 yoctoNEAR, so a leaked function-call key cannot move value.
- **No permanent lock.** Settled transfers are pruned, so unlocks keep working after 128 of them. Before, the 129th unlock failed for everyone. An empty wallet that nothing refers to can give up its slot. A wallet with a withdrawal in flight is never evicted, so a failed transfer is always restored.
- **Init race.** The token and the vesting contract can be initialized only by their own account, inside the deploy transaction.

## Accounts
| Role | Account |
|---|---|
| Token CLIFF (fixed supply, no value) | `ch11b-tok.testnet` |
| Funded vesting contract | `ch11b-vest.testnet` |
| Issuer | `ch11b-iss.testnet` |
| Holder | `ch11b-hold.testnet` |
| Lender | `ch11b-lend.testnet` |

## Transactions
| Step | Tx |
|---|---|
| Deploy and init token in one transaction | 8L5qEFavzNTCKfF2cSR9adrxv18uTHR6h21RorJ9hjXd |
| Deploy and init vesting in one transaction | B2U5y48gLTEyeW1rnCMnQgn49xPAkYry3iDo2jPEZSQp |
| Issuer funds grant g1, 365,000 CLIFF | Cqqps3WwQGcraHxSuzkds1QGZgpgrbCqKTHRPuNrVYUz |
| Lender deposits 200,000 CLIFF as an offer | E7NYNDoSTptN3PVZ1FkRGU2ZsKSug475UbbthvNRjaoD |
| Refused: borrow without 1 yoctoNEAR | "Requires attached deposit of exactly 1 yoctoNEAR" |
| Refused: sell a slice to the issuer, who never offered | "buyer has not offered this much" |
| Refused: a second init | "The contract has already been initialized" |
| Holder borrows 50,000 from the offer | DDjkJo9E5o1Q1cYUouaJVjGzCUSm3NznoGv9DBnwnk6E |
| Unlock: lender paid first (202,712 inside), holder 197,288 | GqStXfrFAz8yYrwcb9mM2kgP5t78H8AWYbFwanBVFfMY |
| Holder withdraws 197,288 CLIFF to own wallet | CVCr2SRYaQUdYWSAtYP5DmnvCprnLd7mWvb9ChHoLxdn |
| **Check:** `backing()` = 367,712 = `ft_balance_of(contract)` | view calls |

## Still true
- The contract account's full-access key can redeploy the code, so whoever holds it can drain the contract. There is no owner method, pause or sweep. Tokens sent with a plain `ft_transfer` are stranded.
- The grant limit is 32 and the wallet limit is 64. Only accounts holding CLIFF can fill them, and the demo keeps the whole supply with the issuer.
- Stake and collect_rewards stay refused in funded mode.

# Agent payday on testnet (12 Nov 2026)

Oscar's ruling, 12 Nov: unlock runs without wallet confirmation so an agent can run vesting payday. Withdraw and every other money move keep the 1 yoctoNEAR confirmation. Reproduce with `AGENT_KEY_FILE=... scripts/deploy-and-prove.sh`. A Codex review found the contract change sound and asked for stricter proof; this run uses it (exact refusal texts, full CLI output kept).

| Role | Account |
|---|---|
| Token CLIFF | `tok.ch11b-iss.testnet` |
| Vesting contract (live site reads this) | `vest.ch11b-iss.testnet` |
| Issuer / holder / lender | `iss.ch11b-hold.testnet` / `hold.ch11b-hold.testnet` / `lend.ch11b-hold.testnet` |

| Step | Tx |
|---|---|
| Deploy and init token | 9GPCKUjRb2dFfs46eUEtG58mayQrwumZF3Cv8zFV7688 |
| Deploy and init vesting | HFRHxJoPnub29xFNW3TCjbH57GS5G9WebRedmSPYeDFw |
| Holder borrows 50,000 from the lender's offer | 4WGnArRnwKhVUGZKzwqMco79cwXy4qZHnqrSEDsL71vh |
| Holder grants the agent a key for `unlock` on the escrow only, 0.25 NEAR gas allowance | BXG7955dy8yw5cQJWEyRkqyDMVrfGN6xPNvuDFUL8rYD |
| **Agent** unlocks g1: lender paid first (202,712), holder credited 197,288 | GkFcibJsrHr6xTe87nh1LcwBWSwJyPWeNYQcNFUcke74 |
| Refused: agent key calls `withdraw` with 1 yoctoNEAR | "Having a deposit with a function call action is not allowed with a function call access key." |
| Refused: agent key calls `withdraw` with no deposit | "Transaction method name <withdraw> isn't allowed by the access key." |
| Holder withdraws 197,288 CLIFF with a wallet-confirmed transaction | 8VLqxQgcNSc9EjsyJdCB5RjyRq8jVvp7ex7hce9w8Hyb |
| **Check:** `backing()` = 367,712 = `ft_balance_of(contract)` | view calls |

The agent can release vested tokens on schedule. It cannot move anything out of the escrow. Both refusals come from the NEAR protocol's access-key rules; the contract's own 1 yoctoNEAR guard on withdraw is covered by the unit test `after_an_agent_unlock_a_no_deposit_withdraw_is_refused`.

Known limit: a leaked agent key can call unlock repeatedly and spend its 0.25 NEAR gas allowance, which stops agent payday until the holder adds a new key. It cannot change who is paid or move tokens.
