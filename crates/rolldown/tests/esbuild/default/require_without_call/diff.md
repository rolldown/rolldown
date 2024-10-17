# Reason
1. should rewrite `require`
# Diff
## /out.js
### esbuild
```js
// entry.js
var req = __require;
req("./entry");
```
### rolldown
```js

//#region entry.js
const req = require;
req("./entry");

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,2 +1,2 @@
-var req = __require;
+var req = require;
 req("./entry");

```