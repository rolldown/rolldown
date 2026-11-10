import defer * as route from './route.js';

globalThis.__events = [];

export function run() {
  const before = [...globalThis.__events];
  const value = route.read();
  return { before, value, events: globalThis.__events };
}
