export const VEST = 'ch11-vest.testnet';
export const METHODS = ['unlock', 'withdraw'];
// The hardened contract asks for 1 yoctoNEAR on withdraw so the wallet must confirm it.
export const WITHDRAW_YOCTO = true;
export function transaction(account, viewed, method, args) {
  if (!account || account !== viewed || !account.endsWith('.testnet')) throw new Error('Read your connected testnet account before signing.');
  if (!METHODS.includes(method)) throw new Error('This desk only signs unlock and withdraw.');
  if (method === 'unlock' && (typeof args.grant_id !== 'string' || !args.grant_id)) throw new Error('Choose a grant.');
  if (method === 'withdraw' && !/^[1-9]\d*$/.test(args.amount)) throw new Error('Enter a positive whole number of CLIFF.');
  return { signerId: account, receiverId: VEST, actions: [{ type: 'FunctionCall', params: { methodName: method, args, gas: '100000000000000', deposit: method === 'withdraw' && WITHDRAW_YOCTO ? '1' : '0' } }] };
}
export function executionError(outcome, method) {
  if (!outcome || typeof outcome.status !== 'object') return 'Waiting for a final execution result.';
  const statuses = [outcome.status, outcome.transaction_outcome?.outcome?.status, ...(outcome.receipts_outcome || []).map(r => r.outcome?.status)].filter(Boolean);
  // withdraw resolves to false when the token transfer failed and its balance was restored.
  if (outcome.status.SuccessValue === 'ZmFsc2U=') return 'Token transfer failed; the contract restored your balance. Refresh before retrying.';
  if (statuses.some(s => s.Failure)) return 'The transaction or a receipt failed. Refresh balances before retrying.';
  if (method === 'withdraw' && outcome.status.SuccessValue !== 'dHJ1ZQ==') return 'Withdrawal completion is unconfirmed. Refresh balances and inspect the transaction before retrying.';
  if (!('SuccessValue' in outcome.status)) return 'Waiting for final receipts. Refresh before retrying.';
  return null;
}
