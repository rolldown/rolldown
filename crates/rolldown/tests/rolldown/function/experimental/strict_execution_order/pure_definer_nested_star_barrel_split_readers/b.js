import * as ns from './outer-barrel.js';

// Second entry sharing the barrel namespace without reading the definer — the split that keeps the
// barrels in a shared chunk and the definer's init reachable only through barrel forwarding.
(globalThis.__events ??= []).push('b');

export const loaded = typeof ns === 'object';
