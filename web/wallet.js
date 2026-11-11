import { setupWalletSelector } from '@near-wallet-selector/core';
import { setupMyNearWallet } from '@near-wallet-selector/my-near-wallet';
import { VEST, transaction, executionError } from './wallet-policy.js';
const $ = id => document.getElementById(id);
let selector, account, snapshot, pending, busy = false;
function say(message) { $('wallet-status').textContent = message; }
function reset() { pending = null; $('wallet-confirm').hidden = true; $('wallet-preview').textContent = ''; }
function available() { return account && snapshot?.holder === account && !busy; }
function render() {
  reset();
  $('wallet-connect').textContent = account ? 'Disconnect testnet wallet' : 'Connect testnet wallet';
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
    pending = transaction(account, snapshot.holder, method, args);
    if (method === 'withdraw' && BigInt(args.amount) > BigInt(snapshot.balance || 0)) throw new Error('Amount exceeds your balance inside Cliffhanger.');
    $('wallet-preview').textContent = `${method === 'unlock' ? 'Unlock ' + args.grant_id + ' into your Cliffhanger balance (lender or buyer paid first)' : 'Withdraw ' + args.amount + ' CLIFF to ' + account}. Contract: ${VEST}. Testnet gas: up to 100 Tgas. Attached NEAR: 0. Confirming requests a signature from your connected testnet wallet.`;
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
  busy = true; render(); say('Waiting for the wallet and final testnet receipts…');
  try {
    const wallet = await selector.wallet();
    if (selector.store.getState().accounts.find(a => a.active)?.accountId !== account) throw new Error('Wallet account changed; reconnect and refresh.');
    // The batch API always routes through MyNearWallet, even if this origin has an old key.
    const outcomes = await wallet.signAndSendTransactions({transactions: [request]});
    let outcome = outcomes?.[0];
    if (outcome?.transaction?.hash) outcome = await finalOutcome(outcome.transaction.hash);
    const error = executionError(outcome, request.actions[0].params.methodName);
    say(error || 'Testnet transaction completed. Reading updated balances…');
    if (outcome?.transaction?.hash) showTx(outcome.transaction.hash);
  } catch (e) { say('No completion confirmed: ' + e.message + '. Refresh before any retry.'); }
  finally { busy = false; await refreshOwn(); }
};
function showTx(hash) {
  if (!/^[1-9A-HJ-NP-Za-km-z]{32,64}$/.test(hash)) return;
  const link = document.createElement('a');
  link.href = 'https://explorer.testnet.near.org/transactions/' + hash; link.target = '_blank'; link.rel = 'noopener'; link.textContent = 'View testnet transaction';
  $('wallet-result').replaceChildren(link);
}
async function finalOutcome(hash) {
  const response = await fetch('https://rpc.testnet.near.org', { method:'POST', headers:{'Content-Type':'application/json'}, body:JSON.stringify({jsonrpc:'2.0',id:'wallet-final',method:'tx',params:{tx_hash:hash,sender_account_id:account,wait_until:'FINAL'}}) });
  const json = await response.json();
  if (!response.ok || json.error) throw new Error(json.error?.message || 'Finality lookup failed');
  return json.result;
}
async function init() {
  try {
    selector = await setupWalletSelector({ network: 'testnet', modules: [setupMyNearWallet({ walletUrl: 'https://testnet.mynearwallet.com' })] });
    function sync(state) {
      const next = state.accounts.find(a => a.active)?.accountId || null;
      if (next !== account) { account = next; snapshot = null; render(); say(account ? `Connected: ${account}. Read your account to enable actions.` : 'Connect your own testnet wallet. Viewing another holder never authorizes an action.'); }
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
    // A return carrying ?deposit= is the mainnet Intents deposit; the page script owns it, not this testnet controller.
    const hashes = params.has('deposit') ? null : params.get('transactionHashes');
    if (hashes && account) {
      const hash = hashes.split(',')[0]; showTx(hash);
      say('Wallet returned a transaction. Checking final receipts…');
      try {
        const outcome = await finalOutcome(hash);
        say(executionError(outcome) || 'Testnet transaction completed. Read your account for updated balances.');
      } catch(e) { say('Could not confirm returned transaction: '+e.message+'. Refresh before retrying.'); }
    }
    render();
  } catch(e) { say('Wallet unavailable: '+e.message+'. Read-only grants remain available.'); }
}
init();
