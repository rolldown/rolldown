import * as ns from './lib/index.js';
console.log('entry', ns.x);
if (globalThis.load) import('./lazy.js');
