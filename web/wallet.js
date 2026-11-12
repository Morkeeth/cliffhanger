import { setupWalletSelector } from '@near-wallet-selector/core';
import { setupMyNearWallet } from '@near-wallet-selector/my-near-wallet';
import { network, transaction, executionError } from './wallet-policy.js';
const $ = id => document.getElementById(id);
// The page script resolves the network before this module runs. No fallback: an unknown value stops the wallet.
const NET = network(window.CLIFF_NET);
let selector, account, snapshot, pending, busy = false;
function say(message) { $('wallet-status').textContent = message; }
function reset() { pending = null; $('wallet-confirm').hidden = true; $('wallet-preview').textContent = ''; }
function available() { return account && snapshot?.holder === account && !busy; }
function render() {
  reset();
  $('wallet-connect').textContent = account ? `Disconnect ${NET.id} wallet` : `Connect ${NET.id} wallet`;
  $('wallet-own').hidden = !account;
  $('wallet-actions').hidden = !available();
  $('wallet-unlock').replaceChildren();
  if (!available()) return;
  for (const option of snapshot.options.filter(o => o.can_claim)) {
    const button = document.createElement('button');
    button.textContent = `Unlock ${option.grant.id}`;
    button.onclick = () => prepare('unlock', { grant_id: option.grant.id });
    $('wallet-unlock').append(button);
  }
  if (!$('wallet-unlock').children.length) $('wallet-unlock').textContent = 'No tokens are available to unlock now.';
  $('wallet-balance').textContent = `${snapshot.balance ?? 0} CLIFF available to withdraw`;
  $('wallet-withdraw').disabled = !snapshot.balance || BigInt(snapshot.balance) <= 0n;
}
function prepare(method, args) {
  try {
    if (!available()) throw new Error('Refresh your connected account first.');
    pending = transaction(NET.id, account, snapshot.holder, method, args);
    if (method === 'withdraw' && BigInt(args.amount) > BigInt(snapshot.balance || 0)) throw new Error('Amount exceeds your balance inside Cliffhanger.');
    $('wallet-preview').textContent = `${method === 'unlock' ? 'Unlock ' + args.grant_id + ' into your Cliffhanger balance (lender or buyer paid first)' : 'Withdraw ' + args.amount + ' CLIFF to ' + account}. Contract: ${NET.vest}. Gas: up to 100 Tgas. ${method === 'withdraw' ? 'Attached: 1 yoctoNEAR, the wallet confirmation that an agent key cannot give.' : 'Attached NEAR: 0, so an unlock-only agent key can sign it too.'} Confirming requests a signature from your connected ${NET.id} wallet.`;
    $('wallet-confirm').hidden = false;
  } catch (e) { reset(); say(e.message); }
}
async function refreshOwn() {
  reset();
  if (account) { $('acct').value = account; $('go').click(); }
}
window.addEventListener('cliffhanger-loading', () => { snapshot = null; render(); });
window.addEventListener('cliffhanger-loaded', event => { snapshot = event.detail; render(); });
$('wallet-own').onclick = refreshOwn;
$('wallet-withdraw').onclick = () => prepare('withdraw', { amount: $('wallet-amount').value.trim() });
$('wallet-confirm').onclick = async () => {
  if (!pending || !available()) return;
  const request = pending;
  // Never automatically resend on redirects or errors.
  busy = true; render(); say(`Waiting for the wallet and final ${NET.id} receipts…`);
  try {
    const wallet = await selector.wallet();
    if (selector.store.getState().accounts.find(a => a.active)?.accountId !== account) throw new Error('Wallet account changed; reconnect and refresh.');
    // The batch API always routes through MyNearWallet, even if this origin has an old key.
    const outcomes = await wallet.signAndSendTransactions({transactions: [request]});
    let outcome = outcomes?.[0];
    if (outcome?.transaction?.hash) outcome = await finalOutcome(outcome.transaction.hash);
    const error = executionError(outcome, request.actions[0].params.methodName);
    say(error || `Transaction completed on ${NET.id}. Reading updated balances…`);
    if (outcome?.transaction?.hash) showTx(outcome.transaction.hash);
  } catch (e) { say('No completion confirmed: ' + e.message + '. Refresh before any retry.'); }
  finally { busy = false; await refreshOwn(); }
};
function showTx(hash) {
  if (!/^[1-9A-HJ-NP-Za-km-z]{32,64}$/.test(hash)) return;
  const link = document.createElement('a');
  link.href = NET.tx + hash; link.target = '_blank'; link.rel = 'noopener'; link.textContent = `View ${NET.id} transaction`;
  $('wallet-result').replaceChildren(link);
}
async function finalOutcome(hash) {
  const response = await fetch(NET.rpc, { method:'POST', headers:{'Content-Type':'application/json'}, body:JSON.stringify({jsonrpc:'2.0',id:'wallet-final',method:'tx',params:{tx_hash:hash,sender_account_id:account,wait_until:'FINAL'}}) });
  const json = await response.json();
  if (!response.ok || json.error) throw new Error(json.error?.message || 'Finality lookup failed');
  return json.result;
}
async function init() {
  try {
    selector = await setupWalletSelector({ network: NET.id, modules: [setupMyNearWallet({ walletUrl: NET.walletUrl })] });
    function sync(state) {
      const next = state.accounts.find(a => a.active)?.accountId || null;
      if (next !== account) { account = next; snapshot = null; render(); say(account ? `Connected: ${account}. Read your account to enable actions.` : `Connect your own ${NET.id} wallet. Viewing another holder never authorizes an action.`); }
    }
    sync(selector.store.getState()); selector.store.observable.subscribe(sync);
    $('wallet-connect').disabled = false;
    $('wallet-connect').onclick = async () => {
      try {
        reset(); const wallet = await selector.wallet('my-near-wallet');
        if (account) { await wallet.signOut(); snapshot = null; render(); }
        else {
          // An empty contractId selects an account without requesting a function-call key.
          // Every transaction therefore goes to the wallet for human approval.
          await wallet.signIn({ contractId: '' });
        }
      } catch (e) { say('Wallet connection did not complete: ' + e.message); }
    };
    const params = new URLSearchParams(location.search);
    if (params.has('errorCode')) say('Wallet declined or failed: ' + (params.get('errorMessage') || params.get('errorCode')));
    // A return carrying ?deposit= is the mainnet Intents deposit; the page script owns it, not this wallet controller.
    const hashes = params.has('deposit') ? null : params.get('transactionHashes');
    if (hashes && account) {
      const hash = hashes.split(',')[0]; showTx(hash);
      say('Wallet returned a transaction. Checking final receipts…');
      try {
        const outcome = await finalOutcome(hash);
        say(executionError(outcome) || `Transaction completed on ${NET.id}. Read your account for updated balances.`);
      } catch(e) { say('Could not confirm returned transaction: '+e.message+'. Refresh before retrying.'); }
    }
    render();
  } catch(e) { say('Wallet unavailable: '+e.message+'. Read-only grants remain available.'); }
}
init();
