import { a } from './a.js';
globalThis.events.push('main ' + a);
globalThis.loadQ = () => import('./q.js');
