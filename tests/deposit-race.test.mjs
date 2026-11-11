// Run: node tests/deposit-race.test.mjs web/index.html (and docs/index.html). Replays a quote change while 1Click answers.
import fs from 'fs';
const src=fs.readFileSync(process.argv[2],'utf8');
const grab=n=>src.match(new RegExp('(async )?function '+n+'\\([\\s\\S]*?\\n\\}\\n'))[0];
const els={};const el=id=>els[id]||(els[id]={id,value:'',innerHTML:'',textContent:'',onclick:null});
globalThis.document={getElementById:el};
globalThis.location={href:'https://x/'};globalThis.localStorage={getItem:()=>null,setItem(){},removeItem(){}};
let release;globalThis.fetch=()=>new Promise(r=>release=r);
const WRAP='nep141:wrap.near', AUR_NEAR='nep141:aurora', ONECLICK='x', WALLET='w';
const E=s=>String(s), validNear=s=>s||'alice.near', validEvm=s=>s;
let LAST=null, SETTLE_SEQ=0;
const depositTx=eval('('+grab('depositTx')+')'); const settle=eval('('+grab('settle')+')');
el('qrefund').value='alice.near';
LAST={origin:WRAP,amount:'1000000000000000000000000',unit:'NEAR',routes:[['AURORA on NEAR',AUR_NEAR]]};
const p=settle('AURORA on NEAR');
// a new quote starts while 1Click is answering (what quote() now does first)
LAST=null; SETTLE_SEQ++; LAST={origin:'nep141:aurora-token',amount:'1000000000000000000',unit:'AURORA',routes:[]};
release({json:async()=>({quote:{depositAddress:'dep.near',amountIn:'1000000000000000000000000',amountInFormatted:'1'}})});
await p;
const out=el('qsettle').innerHTML; console.log(/quote changed/.test(out)?'RACE BLOCKED':'RACE NOT BLOCKED: '+out.slice(0,120));
// happy path still prepares
SETTLE_SEQ=0; LAST={origin:WRAP,amount:'5',unit:'NEAR',routes:[['AURORA on NEAR',AUR_NEAR]]};
const p2=settle('AURORA on NEAR'); release({json:async()=>({quote:{depositAddress:'dep.near',amountIn:'5',amountInFormatted:'5e-24'}})}); await p2;
console.log(/Sign the deposit/.test(el('qsettle').innerHTML)?'HAPPY PATH OK':'HAPPY FAIL '+el('qsettle').innerHTML);
// amount mismatch refused
const p3=settle('AURORA on NEAR'); release({json:async()=>({quote:{depositAddress:'dep.near',amountIn:'6'}})}); await p3;
console.log(/different amount/.test(el('qsettle').innerHTML)?'AMOUNT CHECK OK':'AMOUNT FAIL');
