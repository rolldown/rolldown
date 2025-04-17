# Diff
## /out.js
### esbuild
```js
// test.custom
var require_test = __commonJS({
  "test.custom"(exports, module) {
    module.exports = "#include <stdio.h>";
  }
});

// entry.js
console.log(require_test());
```
### rolldown
```js

//#region rolldown:runtime
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

//#region test.custom
var require_test = __commonJS({ "test.custom"(exports, module) {
	module.exports = "#include <stdio.h>";
} });

//#region entry.js
console.log(require_test());

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
     "test.custom"(exports, module) {
         module.exports = "#include <stdio.h>";
     }

```