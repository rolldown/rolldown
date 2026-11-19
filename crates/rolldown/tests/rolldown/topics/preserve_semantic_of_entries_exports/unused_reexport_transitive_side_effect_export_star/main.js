export * from './a.js';

globalThis.startupValue = globalThis.initialized;
globalThis.entryLog.push('entry');
globalThis.loadLazy = () => import('./lazy.js').then(({ run }) => run());
