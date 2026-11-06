globalThis.events.push('S body');
export let count = 0;
export function bump() {
  count += 1;
  return count;
}
