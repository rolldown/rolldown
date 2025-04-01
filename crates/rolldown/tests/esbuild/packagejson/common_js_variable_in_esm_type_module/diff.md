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



//#region entry.js
var require_entry = __commonJS({ "entry.js"(exports, module) {
	module.exports = null;
} });
//#endregion

export default require_entry();

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,1 +1,6 @@
-module.exports = null;
+var require_entry = __commonJS({
+    "entry.js"(exports, module) {
+        module.exports = null;
+    }
+});
+export default require_entry();

```