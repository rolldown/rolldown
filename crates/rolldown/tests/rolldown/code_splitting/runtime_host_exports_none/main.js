import * as data from './data.js';
globalThis.main = Object.keys(data).join();
globalThis.lazy = import('./lazy.js').then((m) => m.value);
