import * as local from './local.js';
globalThis.mainNamespace = local;
globalThis.pendingChild = import('./lazy.js');
globalThis.pendingChild.catch(() => {});
throw new Error('entry failed');
