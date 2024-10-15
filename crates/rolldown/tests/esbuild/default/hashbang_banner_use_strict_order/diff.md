# Reason
1. hashban not align
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

//#region entry.js
#! in file
foo();

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,6 @@
+
+//#region entry.js
 #! in file
-#! from banner
-"use strict";
-(() => {
-  // entry.js
-  foo();
-})();
\ No newline at end of file
+foo();
+
+//#endregion
\ No newline at end of file

```