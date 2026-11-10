import defer * as lazy from './lazy.js';

export function run() {
  const a = lazy.value;
  const required = require('./required.js');
  return [a, required.value];
}
