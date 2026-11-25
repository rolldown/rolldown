import './vendor.js';
import './b.js';

globalThis.existingCycleLog.push('main');
export const load = () => import('./lib.js');
