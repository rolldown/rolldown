globalThis.log = [];
function pick(a, b) {
  return a;
}
function effect() {
  globalThis.log.push('effect');
  return 'b';
}
export const picked = pick('a', effect());
