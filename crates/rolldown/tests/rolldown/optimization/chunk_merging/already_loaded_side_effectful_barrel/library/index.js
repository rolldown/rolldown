(globalThis.fixtureLog ??= []).push('initialize');
globalThis.fixtureInitialized = true;

export { eagerValueA } from './family-a.js';
export { eagerValueB } from './family-b.js';
export { lazyValueA1, lazyValueA2 } from './family-c.js';
export { lazyValueB } from './family-d.js';
export { lazyValueC } from './family-e.js';
export { unusedValue } from './family-f.js';
