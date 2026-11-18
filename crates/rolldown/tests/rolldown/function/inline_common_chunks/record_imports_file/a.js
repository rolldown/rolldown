import { s1 } from './s1.js';
let helper = 'local';
helper += '';
globalThis.events.push('A ' + helper + ' ' + s1);
