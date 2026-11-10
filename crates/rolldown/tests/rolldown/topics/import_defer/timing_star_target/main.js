import { state } from './state.js';
import defer * as first from './first.js';
import defer * as second from './second.js';

export function run() {
  const a = first.read();
  state.value = 2;
  return [a, second.read()];
}
