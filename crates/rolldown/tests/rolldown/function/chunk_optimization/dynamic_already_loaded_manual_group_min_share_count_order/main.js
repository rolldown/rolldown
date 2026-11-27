import './setup.js';
import './lib.js';

globalThis.minShareCountOrderLog.push('main');
export const load = () => import('./lib.js');
