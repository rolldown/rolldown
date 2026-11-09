import { helper, calls } from './vendor.js';
import { fromAb } from './ab.js';
import { fromAc } from './ac.js';
globalThis.events.push('A ' + fromAb + ' ' + fromAc + ' ' + helper('a') + ' ' + calls);
