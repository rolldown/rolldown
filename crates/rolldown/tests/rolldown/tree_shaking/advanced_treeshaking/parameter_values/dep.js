export function check(x) {
  if (x) globalThis.log.push('DEAD_cross_module');
}
