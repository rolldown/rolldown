# Reason
1. sub optimal
2. redundant wrap function
# Diff
## /out.js
### esbuild
```js
// entry.ts
module.exports = null;
```
### rolldown
```js

//#region rolldown:runtime
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

//#region entry.ts
var require_entry = __commonJS({ "entry.ts"(exports, module) {
	module.exports = null;
} });

export default require_entry();

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,1 +1,12 @@
-module.exports = null;
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __commonJS = (cb, mod) => function () {
+    return (mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = {
+        exports: {}
+    }).exports, mod), mod.exports);
+};
+var require_entry = __commonJS({
+    "entry.ts"(exports, module) {
+        module.exports = null;
+    }
+});
+export default require_entry();

```