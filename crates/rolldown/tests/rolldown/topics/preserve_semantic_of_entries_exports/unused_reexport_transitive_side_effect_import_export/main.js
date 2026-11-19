import { read } from './a.js';
export { read };

globalThis.startupValue = globalThis.initialized;
globalThis.entryLog.push('entry');
globalThis.loadLazy = () => import('./lazy.js').then(({ run }) => run());
