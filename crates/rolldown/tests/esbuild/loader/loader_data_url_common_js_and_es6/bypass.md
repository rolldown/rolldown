# Reason
1. Different codegen order
# Diff
## /out.js
### esbuild
```js
// x.txt
var require_x = __commonJS({
  "x.txt"(exports, module) {
    module.exports = "data:text/plain;charset=utf-8,x";
  }
});

// y.txt
var y_default = "data:text/plain;charset=utf-8,y";

// entry.js
var x_url = require_x();
console.log(x_url, y_default);
```
### rolldown
```js

//#region rolldown:runtime
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

//#region y.txt
var y_default = "data:text/plain;charset=utf-8,y";

//#region x.txt
var require_x = __commonJS({ "x.txt"(exports, module) {
	module.exports = "data:text/plain;charset=utf-8,x";
} });

//#region entry.js
const x_url = require_x();
console.log(x_url, y_default);

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
+var y_default = "data:text/plain;charset=utf-8,y";
 var require_x = __commonJS({
     "x.txt"(exports, module) {
         module.exports = "data:text/plain;charset=utf-8,x";
     }
 });
-var y_default = "data:text/plain;charset=utf-8,y";
 var x_url = require_x();
 console.log(x_url, y_default);

```