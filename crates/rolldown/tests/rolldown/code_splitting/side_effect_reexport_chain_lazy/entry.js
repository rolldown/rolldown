import * as ns from './lib/index.js';
console.log('entry', ns.x);
export let signal = 'ready';
if (globalThis.load) import('./other.js');
