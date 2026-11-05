const esm = require('./esm.js');
exports = { name: 'main', esm: esm.value };
globalThis.lazy = import('./lazy.js').then((m) => m.value);
