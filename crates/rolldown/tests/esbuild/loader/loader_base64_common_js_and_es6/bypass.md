# Reason
1. Different codegen order
# Diff
## /out.js
### esbuild
```js
// x.b64
var require_x = __commonJS({
  "x.b64"(exports, module) {
    module.exports = "eA==";
  }
});

// y.b64
var y_default = "eQ==";

// entry.js
var x_b64 = require_x();
console.log(x_b64, y_default);
```
### rolldown
```js
import assert from "node:assert";

// HIDDEN [rolldown:runtime]
//#region y.b64
var y_default = "eQ==";

//#endregion
//#region x.b64
var require_x = /* @__PURE__ */ __commonJS({ "x.b64"(exports, module) {
	module.exports = "eA==";
} });

//#endregion
//#region entry.js
const x_b64 = require_x();
assert.deepEqual(x_b64, "eA==");
assert.equal(y_default, "eQ==");

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,8 @@
+var y_default = "eQ==";
 var require_x = __commonJS({
     "x.b64"(exports, module) {
         module.exports = "eA==";
     }
 });
-var y_default = "eQ==";
 var x_b64 = require_x();
 console.log(x_b64, y_default);

```