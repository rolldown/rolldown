import { count, inc } from './m.js';
inc();
globalThis.events.push('X ' + count);
