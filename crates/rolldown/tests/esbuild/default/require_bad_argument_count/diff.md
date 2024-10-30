# Reason
1. `__require` rewrite
# Diff
## /out.js
### esbuild
```js
// entry.js
__require();
__require("a", "b");
try {
  __require();
  __require("a", "b");
} catch {
}
```
### rolldown
```js


//#region entry.js
require();
__require("a", "b");
try {
	require();
	__require("a", "b");
} catch {}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,6 @@
-__require();
+require();
 __require("a", "b");
 try {
-    __require();
+    require();
     __require("a", "b");
 } catch {}

```
