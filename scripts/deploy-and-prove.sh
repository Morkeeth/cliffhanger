#!/usr/bin/env bash
# Deploy the CLIFF token and the funded vesting contract, then prove the hardened journey.
# Each contract is deployed and initialized in ONE transaction, so nobody can init it first.
# Usage: NET=testnet TOK=a.testnet VEST=b.testnet ISS=c.testnet HOLD=d.testnet LEND=e.testnet scripts/deploy-and-prove.sh
# Signing keys come from the macOS keychain (near-cli-rs sign-with-keychain). Nothing here prints a key.
set -euo pipefail
: "${NET:?}" "${TOK:?}" "${VEST:?}" "${ISS:?}" "${HOLD:?}" "${LEND:?}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TOKEN_WASM="${TOKEN_WASM:-$ROOT/token/target/wasm32-unknown-unknown/release/cliffhanger_demo_token.wasm}"
VEST_WASM="${VEST_WASM:-$ROOT/contract/target/wasm32-unknown-unknown/release/near_vesting_loan_skeleton.wasm}"
near(){ npx -y near-cli-rs@0.31.1 "$@" </dev/null; }
clean(){ sed 's/\x1b\[[0-9;]*m//g'; }
call(){ # signer contract method json deposit. Retries only network failures (expired or no outcome), never a contract panic.
  local out i
  for i in 1 2 3; do
    out=$(near contract call-function as-transaction "$2" "$3" json-args "$4" prepaid-gas '100.0 Tgas' attached-deposit "$5" sign-as "$1" network-config "$NET" sign-with-keychain send 2>&1 | clean || true)
    if grep -qE "ExecutionError|panicked|Transaction ID" <<<"$out" && ! grep -q "expired" <<<"$out"; then break; fi
    echo "  network retry $i for $3" >&2; sleep 5
  done
  grep -E "Transaction ID|ExecutionError|panicked" <<<"$out" | sort -u | head -3 || true; }
must_fail(){ local out; out=$(call "$@"); if grep -qE "ExecutionError|panicked" <<<"$out"; then echo "  refused as expected: $3 -> $(grep -oE 'panicked: [^\")]*' <<<"$out" | head -1)"; else echo "  !! NOT REFUSED: $3"; echo "$out"; exit 1; fi; }
view(){ near contract call-function as-read-only "$1" "$2" json-args "$3" network-config "$NET" now 2>&1 | clean | grep -A1 "return value" | tail -1 || true; }
TODAY=$(( $(date +%s) / 86400 ))
FROM=${FROM:-1}
step(){ [ "$1" -ge "$FROM" ]; }

if step 1; then
echo "1. Deploy + init token ($TOK), whole fixed supply to $ISS"
near contract deploy "$TOK" use-file "$TOKEN_WASM" with-init-call new json-args "{\"owner_id\":\"$ISS\",\"total_supply\":\"10000000\"}" prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' network-config "$NET" sign-with-keychain send 2>&1 | grep -E "Transaction ID|rror" | head -2
fi
if step 2; then
echo "2. Deploy + init vesting ($VEST) bound to $TOK"
near contract deploy "$VEST" use-file "$VEST_WASM" with-init-call new_funded json-args "{\"token_contract\":\"$TOK\"}" prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' network-config "$NET" sign-with-keychain send 2>&1 | grep -E "Transaction ID|rror" | head -2
fi
if step 3; then
echo "3. Register token storage"
for a in "$VEST" "$HOLD" "$LEND"; do call "$ISS" "$TOK" storage_deposit "{\"account_id\":\"$a\"}" '0.00125 NEAR'; done
fi
if step 4; then
echo "4. Issuer sends the lender 1,000,000 CLIFF"
call "$ISS" "$TOK" ft_transfer "{\"receiver_id\":\"$LEND\",\"amount\":\"1000000\"}" '1 yoctoNEAR'
fi
if step 5; then
echo "5. Issuer funds grant g1: 365,000 CLIFF, started 200 days ago, 30-day cliff, 365 days"
call "$ISS" "$TOK" ft_transfer_call "{\"receiver_id\":\"$VEST\",\"amount\":\"365000\",\"msg\":\"{\\\"action\\\":\\\"create_grant\\\",\\\"grant_id\\\":\\\"g1\\\",\\\"title\\\":\\\"Core\\\",\\\"holder\\\":\\\"$HOLD\\\",\\\"start\\\":$((TODAY-200)),\\\"cliff\\\":30,\\\"duration\\\":365,\\\"cancellable\\\":false}\"}" '1 yoctoNEAR'
fi
if step 6; then
echo "6. Lender deposits 200,000 CLIFF as an explicit offer"
call "$LEND" "$TOK" ft_transfer_call "{\"receiver_id\":\"$VEST\",\"amount\":\"200000\",\"msg\":\"{\\\"action\\\":\\\"offer\\\"}\"}" '1 yoctoNEAR'
echo "   lendable($LEND) = $(view "$VEST" lendable "{\"account_id\":\"$LEND\"}")"
fi
if step 7; then
echo "7. Attacks the review found"
must_fail "$HOLD" "$VEST" borrow "{\"grant_id\":\"g1\",\"amount\":\"50000\",\"lender\":\"$LEND\"}" '0 NEAR'
must_fail "$HOLD" "$VEST" sell_slice "{\"grant_id\":\"g1\",\"slice\":\"10000\",\"price\":\"10000\",\"buyer\":\"$ISS\"}" '1 yoctoNEAR'
must_fail "$HOLD" "$VEST" new_funded "{\"token_contract\":\"$HOLD\"}" '0 NEAR'
fi
if step 8; then
echo "8. Holder borrows 50,000 against g1 (wallet-confirmed, from the offer)"
call "$HOLD" "$VEST" borrow "{\"grant_id\":\"g1\",\"amount\":\"50000\",\"lender\":\"$LEND\"}" '1 yoctoNEAR'
fi
if step 9; then
echo "9. Holder unlocks g1: lender paid first"
call "$HOLD" "$VEST" unlock '{"grant_id":"g1"}' '1 yoctoNEAR'
BAL=$(view "$VEST" demo_balance "{\"account_id\":\"$HOLD\"}" | tr -dc '0-9')
echo "   holder balance inside = $BAL; lender inside = $(view "$VEST" demo_balance "{\"account_id\":\"$LEND\"}")"
fi
if step 10; then
echo "10. Holder withdraws $BAL CLIFF to own wallet"
call "$HOLD" "$VEST" withdraw "{\"amount\":\"$BAL\"}" '1 yoctoNEAR'
fi
if step 11; then
echo "11. Checks"
echo "   holder wallet  = $(view "$TOK" ft_balance_of "{\"account_id\":\"$HOLD\"}")"
echo "   backing()      = $(view "$VEST" backing '{}')"
echo "   token balance  = $(view "$TOK" ft_balance_of "{\"account_id\":\"$VEST\"}")"
fi
