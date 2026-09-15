const { compute } = require('./shared.js');

exports.own = compute(21) + 1;
exports.done = import('./lazy.js').then((m) => m.lazyValue);
