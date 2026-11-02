globalThis.__wrappedEvents.push('dep-anchor');
export let value = globalThis.__wrappedInput;
export function read() {
  return value;
}
export function change() {
  value += 2;
}
export const ready = await Promise.resolve('ready-anchor');
globalThis.__wrappedEvents.push('after-await-anchor');
