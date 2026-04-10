// Second pass: re-bundle the CJS output from first pass
// This simulates importing rolldown-emitted CJS code
const mod = require('./entry1-pass1.js');
console.log(mod.greet());
