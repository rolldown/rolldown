import './a-start.js';
import './b.js';
import './a-end.js';

export const loadA = () => import('./route-a.js');
export const loadB = () => import('./route-b.js');
globalThis.mergeGroupLog.push('main');
