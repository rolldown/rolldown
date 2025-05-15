# Reason
1. different iife wrapper
# Diff
## /out.js
### esbuild
```js
"use 1";
"use 2";
"use 3";
(() => {
  //! 1
  //! 2
  //! 3
  entry();
  //! 4
  //! 5
  //! 6
})();
```
### rolldown
```js
'use 1';
'use 2';
'use 3';


(function() {


//#region entry.js
entry();

//#endregion
})();
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,6 @@
-"use 1";
-"use 2";
-"use 3";
-(() => {
+'use 1';
+'use 2';
+'use 3';
+(function () {
     entry();
 })();

```