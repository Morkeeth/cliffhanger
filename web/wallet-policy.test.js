import test from 'node:test';
import assert from 'node:assert/strict';
import {transaction, executionError, VEST} from './wallet-policy.js';
const own='holder.testnet';
test('signing is tied to the connected holder, testnet, and supported methods',()=>{
  assert.throws(()=>transaction(own,'other.testnet','unlock',{grant_id:'g1'}));
  assert.throws(()=>transaction('holder.near','holder.near','unlock',{grant_id:'g1'}));
  assert.throws(()=>transaction(own,own,'borrow',{}));
  const tx=transaction(own,own,'withdraw',{amount:'42'});
  assert.equal(tx.receiverId,VEST); assert.equal(tx.signerId,own);
  assert.equal(tx.actions[0].params.deposit,'1', 'withdraw carries 1 yoctoNEAR so the wallet must confirm it');
  assert.equal(transaction(own,own,'unlock',{grant_id:'g1'}).actions[0].params.deposit,'1');
  assert.ok(BigInt(tx.actions[0].params.gas)>=60000000000000n);
});
test('token amounts stay exact and reject zero, fractions, negatives and exponent notation',()=>{
  for(const amount of ['0','-1','1.2','1e6',' 12',''])assert.throws(()=>transaction(own,own,'withdraw',{amount}));
  assert.equal(transaction(own,own,'withdraw',{amount:'900719925474099312345'}).actions[0].params.args.amount,'900719925474099312345');
});
test('a top-level success cannot hide FT receipt failure or restored withdrawal',()=>{
  assert.match(executionError({status:{SuccessValue:''},receipts_outcome:[{outcome:{status:{Failure:{error:'FT unregistered'}}}}]}),/receipt failed/);
  assert.match(executionError({status:{SuccessValue:'ZmFsc2U='}}),/restored/);
  assert.match(executionError({status:{SuccessReceiptId:'pending'}}),/Waiting/);
  assert.match(executionError({status:{SuccessValue:''}},'withdraw'),/unconfirmed/);
  assert.equal(executionError({status:{SuccessValue:'dHJ1ZQ=='},receipts_outcome:[{outcome:{status:{SuccessValue:''}}}]}),null);
});
