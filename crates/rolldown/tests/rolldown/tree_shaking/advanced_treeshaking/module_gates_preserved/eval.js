const secret = { flag: false };
export function run() {
  eval('secret.flag = true');
  if (secret.flag) globalThis.log.push('LIVE_eval_write');
}
const viaEval = { a: 'LIVE_eval_read', b: 1 };
export function readEval() {
  return eval('viaEval.a');
}
export let evalFlag = false;
eval('evalFlag = true');
