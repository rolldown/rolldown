# Reason
1. different iife impl
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
'use strict'

(function(exports) {


//#region entry.js
let a = 1;

//#endregion
exports.a = a;
return exports;
})({});
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,5 @@
-"use strict";
-(() => {
-    var a = 1;
-})();
+('use strict')(function (exports) {
+    let a = 1;
+    exports.a = a;
+    return exports;
+})({});

```