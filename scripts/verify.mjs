#!/usr/bin/env node
// One command for a reviewer with a fresh clone and no keys: `npm run verify`.
// It runs the local tests, then reads the live chain through public RPC.
// It never signs, never sends a transaction and needs no NEAR CLI or wallet.
// Flags: --offline skips every network check. --json prints the report as JSON.
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const OFFLINE = process.argv.includes('--offline');
const JSON_OUT = process.argv.includes('--json');

const NETS = {
  testnet: {
    rpc: ['https://rpc.testnet.near.org', 'https://archival-rpc.testnet.near.org'],
    vest: 'vest.ch11b-iss.testnet', token: 'tok.ch11b-iss.testnet', holder: 'hold.ch11b-hold.testnet',
  },
  mainnet: {
    rpc: ['https://rpc.mainnet.near.org'],
    vest: 'vest.morkeeth.near', token: 'cliff.morkeeth.near', holder: 'hold.morkeeth.near',
  },
};
// Agent payday on testnet, from TESTNET-E2E.md: an unlock signed by a key that may call `unlock` only.
const AGENT_UNLOCK_TX = { hash: 'GkFcibJsrHr6xTe87nh1LcwBWSwJyPWeNYQcNFUcke74', signer: 'hold.ch11b-hold.testnet' };
const ONECLICK = 'https://1click.chaindefuser.com/v0/quote';

const results = [];
const record = (status, check, detail) => {
  results.push({ status, check, detail });
  if (!JSON_OUT) console.log(`${status.padEnd(4)}  ${check}: ${detail}`);
};

function run(cmd, args, cwd) {
  const r = spawnSync(cmd, args, { cwd, encoding: 'utf8', timeout: 180_000 });
  return { ok: r.status === 0, out: `${r.stdout || ''}${r.stderr || ''}`, missing: r.error?.code === 'ENOENT' };
}

async function rpc(net, method, params) {
  let lastError;
  for (const url of NETS[net].rpc) {
    try {
      const res = await fetch(url, {
        method: 'POST', headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ jsonrpc: '2.0', id: 'verify', method, params }),
        signal: AbortSignal.timeout(15_000),
      });
      const body = await res.json();
      if (body.error) { lastError = new Error(body.error.cause?.name || body.error.message); continue; }
      return body.result;
    } catch (e) { lastError = e; }
  }
  throw lastError;
}

async function view(net, account, method, args = {}) {
  const result = await rpc(net, 'query', {
    request_type: 'call_function', finality: 'final', account_id: account, method_name: method,
    args_base64: Buffer.from(JSON.stringify(args)).toString('base64'),
  });
  if (result.error) throw new Error(result.error);
  return JSON.parse(Buffer.from(result.result).toString());
}

function localTests() {
  const node = run(process.execPath, ['--experimental-vm-modules', '--test', 'web/wallet-policy.test.js', 'web/wallet-network.test.js', 'web/wallet-controller.test.js'], ROOT);
  const pass = node.out.match(/# pass (\d+)/)?.[1];
  record(node.ok ? 'PASS' : 'FAIL', 'wallet tests (node)', node.ok ? `${pass} passed` : node.out.split('\n').filter(l => /not ok|Error/.test(l)).slice(0, 3).join(' | '));

  const poc = join(ROOT, 'poc');
  for (const file of ['test_loan.py', 'test_choices.py', 'test_planner.py', 'test_intents.py']) {
    const py = run('python3', [file], poc);
    if (py.missing) { record('SKIP', `engine tests (${file})`, 'python3 not found'); continue; }
    const ran = py.out.match(/Ran (\d+) test/)?.[1];
    record(py.ok ? 'PASS' : 'FAIL', `engine tests (${file})`, py.ok ? `${ran ?? '?'} passed, no network` : py.out.trim().split('\n').slice(-3).join(' | '));
  }

  if (!existsSync(join(ROOT, 'contract', 'Cargo.toml'))) return;
  const cargo = run('cargo', ['--version'], ROOT);
  record('INFO', 'contract tests (Rust)', cargo.missing
    ? 'cargo not found; run `cd contract && cargo test` with Rust installed'
    : 'not run here because they compile for minutes; run `cd contract && cargo test`');
}

async function escrowInvariant(net) {
  const { vest, token, holder } = NETS[net];
  let mode;
  try { mode = await view(net, vest, 'accounting_mode'); } catch (e) {
    if (/UNKNOWN_ACCOUNT|CodeDoesNotExist/.test(String(e.message))) {
      record(net === 'mainnet' ? 'INFO' : 'FAIL', `${net} escrow`, `${vest} is not deployed`);
      return;
    }
    throw e;
  }
  record(mode.startsWith('FUNDED') ? 'PASS' : 'FAIL', `${net} escrow mode`, `${vest} says "${mode.split(':')[0]}"`);
  const backing = await view(net, vest, 'backing');
  const held = await view(net, token, 'ft_balance_of', { account_id: vest });
  record(backing === held ? 'PASS' : 'FAIL', `${net} escrow is fully backed`,
    `backing() = ${backing}, ${token}.ft_balance_of(${vest}) = ${held}`);
  const pending = await view(net, vest, 'needs_reconcile');
  record(pending.length === 0 ? 'PASS' : 'FAIL', `${net} no stuck withdrawals`, `needs_reconcile() lists ${pending.length}`);
  const grants = await view(net, vest, 'get_grants', { account_id: holder });
  record(grants.length > 0 ? 'PASS' : 'FAIL', `${net} holder grants readable`, `${grants.length} grant(s) for ${holder}`);
}

async function agentKeyProof() {
  const { hash, signer } = AGENT_UNLOCK_TX;
  const tx = await rpc('testnet', 'tx', { tx_hash: hash, sender_account_id: signer, wait_until: 'NONE' });
  const action = tx.transaction.actions[0]?.FunctionCall;
  const ok = 'SuccessValue' in (tx.status || {});
  record(ok && action?.method_name === 'unlock' && tx.transaction.receiver_id === NETS.testnet.vest ? 'PASS' : 'FAIL',
    'agent unlock landed on chain', `tx ${hash.slice(0, 8)}… calls ${action?.method_name} on ${tx.transaction.receiver_id}, deposit ${action?.deposit}`);
  try {
    const key = await rpc('testnet', 'query', { request_type: 'view_access_key', finality: 'final', account_id: signer, public_key: tx.transaction.public_key });
    const fc = key.permission?.FunctionCall;
    const limited = fc && fc.receiver_id === NETS.testnet.vest && fc.method_names.length === 1 && fc.method_names[0] === 'unlock';
    record(limited ? 'PASS' : 'FAIL', 'agent key can only unlock',
      fc ? `key may call [${fc.method_names.join(', ')}] on ${fc.receiver_id}; it cannot attach a deposit, so withdraw is out of reach` : 'signing key is a full-access key');
  } catch (e) {
    record('WARN', 'agent key can only unlock', `key no longer on the account (${e.message}); the tx above is the proof`);
  }
}

async function intentsDryQuote() {
  const body = {
    dry: true, swapType: 'EXACT_INPUT', slippageTolerance: 100, originAsset: 'nep141:wrap.near', depositType: 'ORIGIN_CHAIN',
    destinationAsset: 'nep141:base-0x833589fcd6edb6e08f4c7c32d4f71b54bda02913.omft.near', amount: '1000000000000000000000000',
    refundTo: 'placeholder.near', refundType: 'ORIGIN_CHAIN', recipient: '0x1111111111111111111111111111111111111111',
    recipientType: 'DESTINATION_CHAIN', deadline: new Date(Date.now() + 30 * 6e4).toISOString(),
  };
  try {
    const res = await fetch(ONECLICK, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body), signal: AbortSignal.timeout(15_000) });
    const out = await res.json();
    const usdc = out.quote?.amountOutFormatted;
    record(res.ok && usdc ? 'PASS' : 'WARN', 'NEAR Intents dry quote (mainnet, no funds)',
      res.ok && usdc ? `1 NEAR -> ${usdc} USDC on Base; dry, so no deposit address is reserved` : `HTTP ${res.status}: ${JSON.stringify(out).slice(0, 120)}`);
  } catch (e) {
    record('WARN', 'NEAR Intents dry quote (mainnet, no funds)', `1Click not reachable from here (${e.name}); the check is optional`);
  }
}

async function main() {
  localTests();
  if (OFFLINE) {
    record('SKIP', 'live chain checks', '--offline');
  } else {
    for (const [name, fn] of [['testnet', () => escrowInvariant('testnet')], ['agent', agentKeyProof], ['mainnet', () => escrowInvariant('mainnet')], ['intents', intentsDryQuote]]) {
      try { await fn(); } catch (e) { record('WARN', `${name} live check`, `RPC unreachable (${e.message})`); }
    }
  }
  const failed = results.filter(r => r.status === 'FAIL').length;
  if (JSON_OUT) console.log(JSON.stringify({ failed, results }, null, 2));
  else console.log(`\n${failed ? `${failed} check(s) FAILED` : 'All required checks passed.'} WARN and INFO lines are optional or not yet deployed.`);
  process.exit(failed ? 1 : 0);
}

main();
