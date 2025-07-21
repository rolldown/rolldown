# Diff
## /out/entry.js
### esbuild
```js
// entry.js
var KEEP1 = x;
var [KEEP2] = [x];
var [KEEP3] = [...{}];
var { KEEP4 } = {};
```
### rolldown
```js
//#region entry.js
x;
var [KEEP2] = [x];
var [KEEP3] = [...{}];
var { KEEP4 } = {};

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,4 +1,4 @@
-var KEEP1 = x;
+x;
 var [KEEP2] = [x];
 var [KEEP3] = [...{}];
 var {KEEP4} = {};

```