globalThis.events.push('shared:init');

export let value = 0;
export function increment() {
  globalThis.events.push(this === undefined ? 'plain-this' : 'bound-this');
  value++;
}
