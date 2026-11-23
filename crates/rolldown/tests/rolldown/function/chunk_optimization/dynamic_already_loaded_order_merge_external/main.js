import './a.js';
import './external.js';
import './b.js';

export const loadA = () => import('./route-a.js');
export const loadB = () => import('./route-b.js');
globalThis.mergeExternalLog.push('main');
