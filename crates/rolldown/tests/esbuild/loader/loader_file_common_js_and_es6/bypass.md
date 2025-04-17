# Reason
1. different hash algorithm
# Diff
## /out.js
### esbuild
```js
// x.txt
var require_x = __commonJS({
  "x.txt"(exports, module) {
    module.exports = "./x-LSAMBFUD.txt";
  }
});

// y.txt
var y_default = "./y-YE5AYNFB.txt";

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
var y_default = "assets/y-DBJZ7rtI.txt";

//#region x.txt
var require_x = __commonJS({ "x.txt"(exports, module) {
	module.exports = "assets/x-BA9VvU_1.txt";
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
+var y_default = "assets/y-DBJZ7rtI.txt";
 var require_x = __commonJS({
     "x.txt"(exports, module) {
-        module.exports = "./x-LSAMBFUD.txt";
+        module.exports = "assets/x-BA9VvU_1.txt";
     }
 });
-var y_default = "./y-YE5AYNFB.txt";
 var x_url = require_x();
 console.log(x_url, y_default);

```