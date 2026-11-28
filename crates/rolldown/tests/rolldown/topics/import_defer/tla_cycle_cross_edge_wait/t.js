import { log } from './log.js';

export var v;
log.push('t start');
await new Promise((r) => setTimeout(r, 5));
v = 'set';
log.push('t end');
