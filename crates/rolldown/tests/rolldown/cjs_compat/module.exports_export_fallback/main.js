// When requiring ESM without module.exports, should get traditional CJS conversion
const result = require('./esm.js');
console.log('result:', result);
console.log('should have __esModule: true, default, foo, bar');
module.exports = result;
