# Diff
## /out.js
### esbuild
```js
"use 1";
"use 2";
"use 3";
(() => {
  // nested.js
  //! A
  //! B
  //! C
  nested();
  //! D
  //! E
  //! F

  // entry.js
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

//#region nested.js
"use A";
"use B";
"use C";
nested();

//#endregion
//#region entry.js
"use 1";
"use 2";
"use 3";
entry();

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,8 @@
+"use A";
+"use B";
+"use C";
+nested();
 "use 1";
 "use 2";
 "use 3";
-(() => {
-    nested();
-    entry();
-})();
+entry();

```