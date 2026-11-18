import { d } from './d.js';
globalThis.events.push('main ' + d);
globalThis.loadE = () => import('./e.js');
