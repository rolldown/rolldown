export function getFoo() {
  return mod.foo;
}
const mod = await import('./top-level-lib.js');
