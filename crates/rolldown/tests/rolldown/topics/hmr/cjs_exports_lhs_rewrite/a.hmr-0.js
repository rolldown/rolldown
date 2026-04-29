exports = module.exports = {};
exports.value = 'v2';
module.exports.other = 'o2';

console.log(exports.value, exports.other);

import.meta.hot.accept();
