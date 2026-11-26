import { outer } from './outer.js';
globalThis.events.push('D ' + outer());
