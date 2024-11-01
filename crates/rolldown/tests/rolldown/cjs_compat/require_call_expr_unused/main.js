require('./esm.js'); // unused
(1, require('./esm.js')); // unused
var a = (1, require("./esm.js")); // Used
require("./esm.js").default; // used
function foo() {
  return require("./esm.js") // Used
}
