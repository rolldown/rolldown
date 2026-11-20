import { log } from './log.js';

// A call, so that the bundler does not inline the values at the reads.
const make = (v) => v;
log.push('a');
export const value = make('a');
export let count = make(1);
export class Klass {}
export function read() {
  return value;
}
