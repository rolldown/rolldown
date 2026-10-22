// The analysis covers a module with a direct `eval` that no included code runs.
function unused() {
  eval('0');
}
const o = { used: 1, dead: 'DEAD_unrelated_eval' };
export function readEval() {
  return o.used;
}
