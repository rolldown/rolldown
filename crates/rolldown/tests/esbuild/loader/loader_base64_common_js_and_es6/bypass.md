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

//#region rolldown:runtime
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

//#region y.b64
var y_default = "eQ==";

//#region x.b64
var require_x = __commonJS({ "x.b64"(exports, module) {
	module.exports = "eA==";
} });

//#region entry.js
const x_b64 = require_x();
assert.deepEqual(x_b64, "eA==");
assert.equal(y_default, "eQ==");

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,14 @@
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __commonJS = (cb, mod) => function () {
+    return (mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = {
+        exports: {}
+    }).exports, mod), mod.exports);
+};
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