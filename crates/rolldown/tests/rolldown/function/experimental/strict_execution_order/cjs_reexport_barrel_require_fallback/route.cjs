const ns = require('required-barrel');

globalThis.__events.push('route');

module.exports = ns.clone({ value: 1 });
