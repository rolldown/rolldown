# Diff
## /out.js
### esbuild
```js
// required.cjs
var require_required = __commonJS({
  "required.cjs"() {
    console.log("works");
  }
});

// entry.ts
require_required();
```
### rolldown
```js

//#region rolldown:runtime
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

//#region required.cjs
var require_required = __commonJS({ "required.cjs"() {
	console.log("works");
} });

//#region entry.ts
require_required();

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
 var require_required = __commonJS({
     "required.cjs"() {
         console.log("works");
     }

```