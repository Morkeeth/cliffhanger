import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import vm from 'node:vm';
import * as policy from './wallet-policy.js';
const {NETWORKS, network, transaction} = policy;

const pages = {};
for (const page of ['web/index.html', 'docs/index.html']) pages[page] = await fs.readFile(new URL('../' + page, import.meta.url), 'utf8');
const block = src => src.match(/\/\/ Network switch\.[\s\S]*?\/\/ end network switch\n/)[0];
const grab = (src, name) => src.match(new RegExp('(async )?function ' + name + '\\([\\s\\S]*?\\n\\}\\n'))[0];
// Runs the real config block from a page with a given query string.
function resolve(src, search) {
  const window = {};
  return new Function('location', 'window', block(src) + 'return {DEFAULT_NET, NETS, NET_NAME, NET, flag: window.CLIFF_NET};')({search}, window);
}

test('both pages default to testnet and only ?net=mainnet switches', () => {
  for (const [page, src] of Object.entries(pages)) {
    assert.equal(resolve(src, '').DEFAULT_NET, 'testnet', page);
    for (const search of ['', '?net=', '?net=garbage', '?net=__proto__', '?net=constructor', '?net=MAINNET', '?account=x.testnet'])
      assert.equal(resolve(src, search).NET_NAME, 'testnet', page + ' ' + search);
    const main = resolve(src, '?net=mainnet&account=hold.morkeeth.near');
    assert.equal(main.NET_NAME, 'mainnet'); assert.equal(main.flag, 'mainnet');
    assert.equal(main.NET.vest, 'vest.morkeeth.near'); assert.equal(main.NET.token, 'cliff.morkeeth.near');
    assert.equal(resolve(src, '?net=testnet').NET.vest, 'vest.ch11b-iss.testnet');
  }
});

test('the two pages and the wallet policy name the same contracts', () => {
  assert.equal(block(pages['web/index.html']), block(pages['docs/index.html']), 'config blocks must be identical');
  const {NETS} = resolve(pages['web/index.html'], '');
  assert.deepEqual(Object.keys(NETS).sort(), Object.keys(NETWORKS).sort());
  for (const id of Object.keys(NETWORKS)) for (const key of ['vest', 'rpc', 'tx']) assert.equal(NETS[id][key], NETWORKS[id][key], id + '.' + key);
  for (const src of Object.values(pages)) {
    // No contract name or RPC outside the switch, except the always-mainnet Intents RPC.
    const rest = src.replace(block(src), '');
    assert.doesNotMatch(rest, /ch11b|morkeeth\.near|rpc\.testnet/);
  }
});

test('signing refuses an account from the other network, both ways', () => {
  assert.throws(() => transaction('mainnet', 'holder.testnet', 'holder.testnet', 'unlock', {grant_id: 'g1'}), /mainnet account/);
  assert.throws(() => transaction('testnet', 'holder.near', 'holder.near', 'unlock', {grant_id: 'g1'}), /testnet account/);
  assert.throws(() => transaction(undefined, 'holder.testnet', 'holder.testnet', 'unlock', {grant_id: 'g1'}), /Unknown NEAR network/);
  assert.throws(() => network('__proto__'), /Unknown NEAR network/);
  const tx = transaction('mainnet', 'holder.near', 'holder.near', 'withdraw', {amount: '5'});
  assert.equal(tx.receiverId, 'vest.morkeeth.near'); assert.equal(tx.actions[0].params.deposit, '1');
});

test('mainnet reads say "not deployed yet" while the contracts do not exist', async () => {
  for (const [page, src] of Object.entries(pages)) {
    // Payloads captured from rpc.mainnet.near.org on 12 Nov 2026.
    const missing = {jsonrpc: '2.0', error: {name: 'HANDLER_ERROR', cause: {name: 'UNKNOWN_ACCOUNT'}, code: -32000, message: 'Server error', data: 'account vest.morkeeth.near does not exist while viewing'}};
    const noCode = {jsonrpc: '2.0', result: {error: 'wasm execution failed with error: CompilationError(CodeDoesNotExist { account_id: "vest.morkeeth.near" })', logs: []}};
    const other = {jsonrpc: '2.0', result: {error: 'wasm execution failed with error: MethodNotFound'}};
    for (const [reply, expect] of [[missing, true], [noCode, true], [other, false]]) {
      const view = new Function('fetch', 'RPC', 'NET_NAME', grab(src, 'notDeployed') + grab(src, 'view') + 'return view;')(async () => ({json: async () => reply}), 'https://rpc.mainnet.near.org', 'mainnet');
      const error = await view('vest.morkeeth.near', 'backing').then(() => null, e => e);
      assert.ok(error, page);
      assert.equal(!!error.notDeployed, expect, page + ' ' + error.message);
      if (expect) assert.match(error.message, /vest\.morkeeth\.near is not deployed yet on NEAR mainnet/);
    }
    assert.match(src, /e\.notDeployed\?'Not deployed yet\. '\+e\.message/, page + ' shows the not-deployed message');
  }
});

async function controller(net) {
  const nodes = new Map();
  const $ = id => { if (!nodes.has(id)) nodes.set(id, {hidden: false, disabled: false, textContent: '', value: '', children: [], replaceChildren(...n) { this.children = n }, append(n) { this.children.push(n) }}); return nodes.get(id); };
  const seen = {};
  const selector = {wallet: async () => ({}), store: {getState: () => ({accounts: []}), observable: {subscribe() {}}}};
  const context = vm.createContext({console, URLSearchParams, BigInt, location: {search: ''}, document: {getElementById: $, createElement: () => ({})}, window: {CLIFF_NET: net, addEventListener() {}}, fetch: async () => ({})});
  const module = new vm.SourceTextModule(await fs.readFile(new URL('./wallet.js', import.meta.url), 'utf8'), {context});
  await module.link(async spec => {
    const exports = spec.includes('wallet-policy') ? policy : spec.includes('my-near-wallet') ? {setupMyNearWallet: o => { seen.walletUrl = o.walletUrl; return {} }} : {setupWalletSelector: async o => { seen.network = o.network; return selector }};
    return new vm.SyntheticModule(Object.keys(exports), function () { for (const [k, v] of Object.entries(exports)) this.setExport(k, v) }, {context});
  });
  await module.evaluate(); await new Promise(r => setImmediate(r));
  return {seen, $};
}

test('the wallet controller follows the page network', async () => {
  const main = await controller('mainnet');
  assert.deepEqual(main.seen, {walletUrl: 'https://app.mynearwallet.com', network: 'mainnet'});
  assert.equal(main.$('wallet-connect').textContent, 'Connect mainnet wallet');
  const testnet = await controller('testnet');
  assert.deepEqual(testnet.seen, {walletUrl: 'https://testnet.mynearwallet.com', network: 'testnet'});
  await assert.rejects(controller(undefined), /Unknown NEAR network/);
});

test('each full inline page script compiles', () => {
  for (const [page, src] of Object.entries(pages)) {
    const inline = [...src.matchAll(/<script>([\s\S]*?)<\/script>/g)].map(m => m[1]);
    assert.ok(inline.length >= 1, page);
    for (const code of inline) assert.doesNotThrow(() => new vm.Script(code), page);
  }
});

test('no visible copy says testnet except through the network name', () => {
  for (const [page, src] of Object.entries(pages)) {
    const rest = src.replace(block(src), '').replace(/<span class="netname">testnet<\/span>/g, '').replace(/<span class="tn-only">[\s\S]*?<\/span>/g, '').replace(/<title>[^<]*<\/title>/, '').replace(/NET_NAME[!=]==?'testnet'/g, '');
    assert.doesNotMatch(rest, /testnet/i, page);
  }
});
