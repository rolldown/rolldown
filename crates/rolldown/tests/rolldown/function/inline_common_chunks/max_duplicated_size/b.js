import { outer } from './outer.js';
import { small } from './small.js';
globalThis.events.push('B ' + outer() + ' ' + small);
