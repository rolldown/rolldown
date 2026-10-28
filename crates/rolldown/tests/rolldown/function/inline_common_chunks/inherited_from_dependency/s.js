globalThis.events.push('S body');
export let n = 0;
export function bump() {
  n += 1;
  return n;
}
