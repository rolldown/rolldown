import { value } from './vendor.js';

globalThis.manualGroupCycleLog.push(value);
export const loadLazy = () => import('./lazy.js').then((m) => m.v);
