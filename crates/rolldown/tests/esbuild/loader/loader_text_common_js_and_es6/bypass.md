# Reason
1. Different codegen order
# Diff
## /out.js
### esbuild
```js
// x.txt
var require_x = __commonJS({
  "x.txt"(exports, module) {
    module.exports = "x";
  }
});

// y.txt
var y_default = "y";

// entry.js
var x_txt = require_x();
console.log(x_txt, y_default);
```
### rolldown
```js
import assert from "node:assert";

// HIDDEN [rolldown:runtime]
//#region y.txt
var y_default = "y";

//#endregion
//#region x.txt
var require_x = /* @__PURE__ */ __commonJS({ "x.txt"(exports, module) {
	module.exports = "x";
} });

//#endregion
//#region entry.js
const x_txt = require_x();
assert.equal(x_txt, "x");
assert.equal(y_default, "y");

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,8 @@
+var y_default = "y";
 var require_x = __commonJS({
     "x.txt"(exports, module) {
         module.exports = "x";
     }
 });
-var y_default = "y";
 var x_txt = require_x();
 console.log(x_txt, y_default);

```