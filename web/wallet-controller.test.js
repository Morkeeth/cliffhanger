import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import vm from 'node:vm';
import * as policy from './wallet-policy.js';

test('actual controller: account-only connect, unlock, withdrawal failure, identity switch and no automatic retry',async()=>{
  const nodes=new Map();
  function element(){return {hidden:false,disabled:false,textContent:'',value:'',children:[],replaceChildren(...n){this.children=n},append(n){this.children.push(n)}}}
  const $=id=>{if(!nodes.has(id))nodes.set(id,element());return nodes.get(id)};
  const listeners={}; let storeListener; let account='holder.testnet'; let sent=[]; let signInArgs; let reads=0;
  let result={status:{SuccessValue:''},transaction:{hash:'A'.repeat(44)},receipts_outcome:[]};
  const wallet={signIn:async args=>{signInArgs=args},signOut:async()=>{},signAndSendTransactions:async args=>{sent.push(args);return [result]}};
  const selector={wallet:async()=>wallet,store:{getState:()=>({accounts:account?[{active:true,accountId:account}]:[]}),observable:{subscribe:fn=>{storeListener=fn}}}};
  $('go').click=()=>{reads++};
  const context=vm.createContext({console,URLSearchParams,BigInt,location:{search:'',origin:'http://localhost',pathname:'/'},document:{getElementById:$,createElement:element},window:{CLIFF_NET:'testnet',addEventListener:(name,fn)=>{listeners[name]=fn}},fetch:async()=>({ok:true,json:async()=>({result})})});
  const source=await fs.readFile(new URL('./wallet.js',import.meta.url),'utf8');
  const module=new vm.SourceTextModule(source,{context});
  await module.link(async spec=>{
    const exports=spec.includes('wallet-policy')?policy:spec.includes('my-near-wallet')?{setupMyNearWallet:()=>({})}:{setupWalletSelector:async opts=>{assert.equal(opts.network,'testnet');return selector}};
    return new vm.SyntheticModule(Object.keys(exports),function(){for(const [key,value]of Object.entries(exports))this.setExport(key,value)},{context});
  });
  await module.evaluate(); await new Promise(resolve=>setImmediate(resolve));
  assert.equal(sent.length,0); assert.equal($('wallet-actions').hidden,true);
  const load=(holder=account,balance='42')=>listeners['cliffhanger-loaded']({detail:{holder,balance,options:[{can_claim:true,grant:{id:'g1'}}]}});
  load('other.testnet'); assert.equal($('wallet-actions').hidden,true);
  load(); assert.equal($('wallet-actions').hidden,false);
  $('wallet-unlock').children[0].onclick(); assert.equal(sent.length,0); assert.equal($('wallet-confirm').hidden,false);
  await $('wallet-confirm').onclick(); assert.equal(sent.length,1); assert.equal(sent[0].transactions[0].actions[0].params.methodName,'unlock');assert.equal(reads,1);
  load(); $('wallet-amount').value='43';$('wallet-withdraw').onclick();assert.match($('wallet-status').textContent,/exceeds/);assert.equal($('wallet-confirm').hidden,true);
  $('wallet-amount').value='42';$('wallet-withdraw').onclick();
  result={status:{SuccessValue:'ZmFsc2U='},transaction:{hash:'B'.repeat(44)}};
  await $('wallet-confirm').onclick();assert.equal(sent.length,2);assert.match($('wallet-status').textContent,/restored/);
  await $('wallet-confirm').onclick();assert.equal(sent.length,2);
  load(); $('wallet-unlock').children[0].onclick(); account='changed.testnet';storeListener(selector.store.getState());await $('wallet-confirm').onclick();assert.equal(sent.length,2);
  account=null;storeListener(selector.store.getState());await $('wallet-connect').onclick();assert.equal(signInArgs.contractId,'');assert.equal(signInArgs.methodNames,undefined);
  assert.equal(sent.length,2);
});
