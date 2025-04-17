# Reason
1. redundant `__commonJS` wrapper
# Diff
## /out.js
### esbuild
```js
// entry.js
module.exports = null;
```
### rolldown
```js

//#region rolldown:runtime
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

//#region entry.js
var require_entry = __commonJS({ "entry.js"(exports, module) {
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
+    "entry.js"(exports, module) {
+        module.exports = null;
+    }
+});
+export default require_entry();

```