export { read } from './forward.js';

globalThis.startupValue = globalThis.initialized;
globalThis.entryLog.push('entry');
globalThis.loadLazy = () => import('./lazy.js').then(({ run }) => run());
