# Reason
1. iife impl follows rollup
# Diff
## /out.js
### esbuild
```js
#! in file
#! from banner
"use strict";
(() => {
  // entry.js
  foo();
})();
```
### rolldown
```js
#! from banner
(function() {

"use strict";

//#region entry.js
foo();

//#endregion
})();
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,10 @@
-#! in file
 #! from banner
+(function() {
+
 "use strict";
-(() => {
-  // entry.js
-  foo();
+
+//#region entry.js
+foo();
+
+//#endregion
 })();
\ No newline at end of file

```