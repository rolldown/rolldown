(globalThis.__events ??= []).push('a');
const b = require('./b.cjs');
exports.name = 'a';
exports.seen = 'a saw b.name=' + b.name;
