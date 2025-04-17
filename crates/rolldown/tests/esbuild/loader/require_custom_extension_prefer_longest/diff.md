# Diff
## /out.js
### esbuild
```js
// test.txt
var require_test = __commonJS({
  "test.txt"(exports, module) {
    module.exports = "test.txt";
  }
});

// test.base64.txt
var require_test_base64 = __commonJS({
  "test.base64.txt"(exports, module) {
    module.exports = "dGVzdC5iYXNlNjQudHh0";
  }
});

// entry.js
console.log(require_test(), require_test_base64());
```
### rolldown
```js

//#region rolldown:runtime
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

//#region test.txt
var require_test = __commonJS({ "test.txt"(exports, module) {
	module.exports = "test.txt";
} });

//#region test.base64.txt
var require_test_base64 = __commonJS({ "test.base64.txt"(exports, module) {
	module.exports = "dGVzdC5iYXNlNjQudHh0";
} });

//#region entry.js
console.log(require_test(), require_test_base64());

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,10 @@
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __commonJS = (cb, mod) => function () {
+    return (mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = {
+        exports: {}
+    }).exports, mod), mod.exports);
+};
 var require_test = __commonJS({
     "test.txt"(exports, module) {
         module.exports = "test.txt";
     }

```