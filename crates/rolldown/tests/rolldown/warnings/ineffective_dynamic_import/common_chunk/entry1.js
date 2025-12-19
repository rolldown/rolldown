// Entry 1: imports lib statically and dynamically
import { lib } from './lib.js';
import('./lib.js').then(mod => console.log('Dynamic in entry1:', mod.lib));
console.log('Entry 1:', lib);
