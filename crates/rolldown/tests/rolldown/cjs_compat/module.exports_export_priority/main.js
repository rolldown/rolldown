// Verify that module.exports takes priority
import result from './cjs.js';
console.log('result:', result);
console.log('should be { priorityValue: "module.exports wins" }');
console.log('not { namedExport: "named", default: ... }');
