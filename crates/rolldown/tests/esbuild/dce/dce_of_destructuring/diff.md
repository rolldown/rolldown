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

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,4 +0,0 @@
-var KEEP1 = x;
-var [KEEP2] = [x];
-var [KEEP3] = [...{}];
-var {KEEP4} = {};

```