import { d } from './d.js';
globalThis.events.push('main ' + d);
globalThis.loadD = () => import('./d.js');
globalThis.loadP = () => import('./p.js');
