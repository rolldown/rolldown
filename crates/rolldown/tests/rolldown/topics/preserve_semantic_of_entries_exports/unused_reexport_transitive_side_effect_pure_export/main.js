export { value } from './pure/index.js';

globalThis.startupValue = globalThis.initialized;
globalThis.entryLog.push('entry');
globalThis.loadLazy = () => import('./lazy.js').then(({ run }) => run());
