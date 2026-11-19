import defer * as first from './first.js';
import defer * as second from './second.js';

globalThis.__events = [];

export function run() {
  const before = [...globalThis.__events];
  const firstValue = first.read();
  const middle = [...globalThis.__events];
  const secondValue = second.read();
  return { before, first: firstValue, middle, second: secondValue, events: globalThis.__events };
}
