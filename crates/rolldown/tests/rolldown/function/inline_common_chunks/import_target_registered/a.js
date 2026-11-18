import { count, inc } from './m.js';
inc();
globalThis.events.push('A ' + count);
globalThis.loadFromA = () => import('./m.js');
