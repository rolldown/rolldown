import { outer } from './outer.js';
globalThis.events.push('C ' + outer());
