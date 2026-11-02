(globalThis.__events ??= []).push('b');
const a = require('./a.cjs');
exports.name = 'b';
exports.seen = 'b saw a.name=' + a.name;
