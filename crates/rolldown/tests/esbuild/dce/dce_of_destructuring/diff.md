# Reason
1. side effects detect
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
var KEEP1 = x;
var [remove3] = [];
var [remove4, ...remove5] = [...[1, 2], 3];
var [, , remove6] = [
	,
	,
	3
];
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
@@ -1,4 +1,7 @@
 var KEEP1 = x;
+var [remove3] = [];
+var [remove4, ...remove5] = [...[1, 2], 3];
+var [, , remove6] = [, , 3];
 var [KEEP2] = [x];
 var [KEEP3] = [...{}];
 var {KEEP4} = {};

```