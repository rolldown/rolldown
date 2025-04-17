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

//#region rolldown:runtime
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

//#region y.txt
var y_default = "y";

//#region x.txt
var require_x = __commonJS({ "x.txt"(exports, module) {
	module.exports = "x";
} });

//#region entry.js
const x_txt = require_x();
assert.equal(x_txt, "x");
assert.equal(y_default, "y");

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