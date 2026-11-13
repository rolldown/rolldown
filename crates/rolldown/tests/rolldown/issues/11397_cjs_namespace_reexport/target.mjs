import { bump } from './a.cjs';

export let count = 0;
export function increment() {
  bump();
  return ++count;
}
// A second import record for the same CommonJS module. The chunk merges its namespace variable
// with the one of the import above.
export * as ns from './a.cjs';
