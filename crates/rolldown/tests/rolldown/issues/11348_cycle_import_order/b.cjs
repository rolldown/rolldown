(globalThis.__events ??= []).push('b');
require('./a.cjs');
exports.name = 'b';
