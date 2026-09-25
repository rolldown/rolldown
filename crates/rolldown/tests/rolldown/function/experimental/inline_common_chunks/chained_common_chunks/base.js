globalThis.events.push('base:init');

let value = 0;
export function next() {
  return ++value;
}
