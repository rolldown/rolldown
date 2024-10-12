# Diff
## /out.js
### esbuild
```js
// entry.ts
module.exports = null;
```
### rolldown
```js


//#region entry.ts
var require_entry = __commonJS({ "entry.ts"(exports, module) {
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
+    "entry.ts"(exports, module) {
+        module.exports = null;
+    }
+});
+export default require_entry();

```