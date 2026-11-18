export let n = 5;
export function bump() {
  return ++n;
}
globalThis.__event('leaf0');
