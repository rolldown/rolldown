export let n = 20;
export const tag = 'leaf1';
export function bump() {
  return ++n;
}
globalThis.__event('leaf1');
