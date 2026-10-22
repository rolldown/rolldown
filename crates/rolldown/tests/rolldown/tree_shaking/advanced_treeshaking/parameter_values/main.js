import { check } from './dep.js';

globalThis.log = [];
const log = (msg) => globalThis.log.push(msg);

// The value of a parameter goes to the return value.
function identity(value) {
  return value;
}
const flag = identity(false);
export function main() {
  if (flag) log('DEAD_identity');
  return 1;
}
main();

// All calls give the same value.
function branch(debug) {
  if (debug) log('DEAD_branch');
  return 1;
}
branch(false);
branch(false);

// A property of an object argument.
function readConfig(config) {
  if (config.verbose) log('DEAD_property');
}
readConfig({ verbose: false, unused: 'DEAD_unused_property' });

// A destructured parameter.
function destructure({ mode }) {
  if (mode !== 'production') log('DEAD_destructure');
}
destructure({ mode: 'production' });

// A parameter goes to another function.
function outer(x) {
  inner(x);
}
function inner(y) {
  if (y) log('DEAD_forward');
}
outer(false);

// A call to a function of another module.
check(false);

export const done = true;
