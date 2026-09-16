globalThis.events.push('S body');
export let count = 0;
export function bump() {
  count += 1;
  return this === undefined ? 'this:undefined' : 'this:bound';
}
export const marker = { tag: 'shared' };
