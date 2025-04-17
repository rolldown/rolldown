# Diff
## /out.js
### esbuild
```js
// b.js
var require_b = __commonJS({
  "b.js"(exports) {
    exports.x = 123;
  }
});

// a.js
console.log(require_b());
console.log(require_b());
```
### rolldown
```js

//#region rolldown:runtime
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

//#region b.js
var require_b = __commonJS({ "b.js"(exports) {
	exports.x = 123;
} });

//#region a.js
console.log(require_b());
console.log(require_b());

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	a.js
@@ -1,4 +1,10 @@
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __commonJS = (cb, mod) => function () {
+    return (mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = {
+        exports: {}
+    }).exports, mod), mod.exports);
+};
 var require_b = __commonJS({
     "b.js"(exports) {
         exports.x = 123;
     }

```