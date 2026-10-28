const b = require('./b.cjs');
exports.name = 'a';
exports.other = () => b.name;
exports.partial = b.partial;
