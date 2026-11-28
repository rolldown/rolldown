import { log } from './log.js';
import './a.js';

export let x = 'unset';
log.push('b start');
await new Promise((r) => setTimeout(r, 10));
x = 'set';
log.push('b end');
