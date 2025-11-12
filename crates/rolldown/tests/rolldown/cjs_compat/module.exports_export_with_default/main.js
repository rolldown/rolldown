// Test that module.exports takes precedence over default export
const result = require('./esm.js');
console.log('result:', result);
console.log('should be { customExport: "custom" }');
module.exports = result;
