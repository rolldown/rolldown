import { count, inc } from './m.js';
inc();
globalThis.events.push('B ' + count);
