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
(function() {

"use strict";

//#region nested.js
"use A";
"use B";
"use C";
nested();

//#region entry.js
"use 1";
"use 2";
"use 3";
entry();

})();
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,10 @@
-"use 1";
-"use 2";
-"use 3";
-(() => {
+(function () {
+    "use A";
+    "use B";
+    "use C";
     nested();
+    "use 1";
+    "use 2";
+    "use 3";
     entry();
 })();

```