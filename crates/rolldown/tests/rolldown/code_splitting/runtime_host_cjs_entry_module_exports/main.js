const esm = require('./esm.js');
module.exports = { name: 'main', esm: esm.value };
globalThis.lazy = import('./lazy.js').then((m) => m.value);
