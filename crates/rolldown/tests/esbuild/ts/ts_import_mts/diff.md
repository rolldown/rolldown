# Diff
## /out.js
### esbuild
```js
// imported.mts
console.log("works");
```
### rolldown
```js
import "./imported.mjs";


```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,1 +1,1 @@
-console.log("works");
+import "./imported.mjs";

```