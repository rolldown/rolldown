import { a } from './a.js';
import { d } from './d.js';
globalThis.events.push('main ' + a + d);
globalThis.loadQ = () => import('./q.js');
