globalThis.log = [];
const log = (msg) => globalThis.log.push(msg);

// Array values fold branches: an element, a nested path, a returned array, and an argument.
const flags = [false];
const nested = [{ debug: false }];
function make() {
  return [false];
}
const made = make();
function first(array) {
  return array[0];
}
const fromArgument = first([false]);
export function main() {
  if (flags[0]) log('DEAD_index');
  if (nested[0].debug) log('DEAD_nested');
  if (made[0]) log('DEAD_return');
  if (fromArgument) log('DEAD_argument');
  return 1;
}

// An element decides a dynamic import.
export async function load() {
  if (flags[0]) return import('./dead.js');
  return 1;
}

// Array methods without effects.
['DEAD_filter'].filter(() => true);
['DEAD_for_each'].forEach(() => {});
['DEAD_includes'].includes('x');
['DEAD_join'].join(',');
['DEAD_map'].map((x) => x);
['DEAD_slice'].slice();
const pushed = [];
pushed.push('DEAD_push');
