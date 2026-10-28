(globalThis.__events ??= []).push('a');
const b = require('./b.cjs');
exports.seen = b.name;
