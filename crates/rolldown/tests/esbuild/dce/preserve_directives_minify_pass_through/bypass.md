# Reason
1. different quote style did not matter, rolldown just use string slice from original code without any format
# Diff
## /out.js
### esbuild
```js
"use 1";
"use 2";
"use 3";
//! 1
//! 2
//! 3
entry();
//! 4
//! 5
//! 6
```
### rolldown
```js
'use 1';
'use 2';
'use 3';


//#region entry.js
entry();

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,4 @@
-"use 1";
-"use 2";
-"use 3";
+'use 1';
+'use 2';
+'use 3';
 entry();

```