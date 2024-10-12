# Diff
## /out.js
### esbuild
```js
"use strict";
(() => {
  // entry.js
  var a = 1;
})();
```
### rolldown
```js

//#region entry.js
let a = 1;

//#endregion
export { a };

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,2 @@
-"use strict";
-(() => {
-    var a = 1;
-})();
+var a = 1;
+export {a};

```