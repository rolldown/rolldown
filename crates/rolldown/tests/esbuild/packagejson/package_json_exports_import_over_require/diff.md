# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/node_modules/pkg/import.js
console.log("SUCCESS");
```
### rolldown
```js



//#region node_modules/pkg/require.js
var require_require = __commonJS({ "node_modules/pkg/require.js"() {
	console.log("FAILURE");
} });
//#endregion

//#region src/entry.js
require_require();
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	entry.js
@@ -1,1 +1,6 @@
-console.log("SUCCESS");
+var require_require = __commonJS({
+    "node_modules/pkg/require.js"() {
+        console.log("FAILURE");
+    }
+});
+require_require();

```