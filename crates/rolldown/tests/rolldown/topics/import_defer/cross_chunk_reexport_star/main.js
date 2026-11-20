import { state } from './state.js';
import defer * as first from './first.js';

state.value = 3;

export function run() {
  const a = first.read();
  state.value = 2;
  return a;
}
