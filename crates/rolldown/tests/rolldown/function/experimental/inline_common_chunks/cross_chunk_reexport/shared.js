globalThis.events.push('shared:init');

export let value = 0;
export function increment() {
  return ++value;
}
