globalThis.log = [];
function effect() {
  globalThis.log.push('effect');
  return 2;
}
const o = { a: 1, b: effect() };
export const value = o.a;
