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
require("a", "b");
try {
	require();
	require("a", "b");
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
-__require("a", "b");
+require();
+require("a", "b");
 try {
-    __require();
-    __require("a", "b");
+    require();
+    require("a", "b");
 } catch {}

```