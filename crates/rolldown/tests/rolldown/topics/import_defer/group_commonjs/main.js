import './setup.js';
import shared from './shared.cjs';
import defer * as route from './route.js';

export function run() {
  const value = route.read();
  return { shared, value, lazy: route.lazy(), events: globalThis.__events };
}
