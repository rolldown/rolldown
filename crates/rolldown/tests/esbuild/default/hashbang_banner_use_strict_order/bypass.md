# Reason
1. `"use strict"` generation follows rollup
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
#! in file
#! from banner
'use strict';


(function() {


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
@@ -1,7 +1,13 @@
 #! in file
 #! from banner
-"use strict";
-(() => {
-  // entry.js
-  foo();
+'use strict';
+
+
+(function() {
+
+
+//#region entry.js
+foo();
+
+//#endregion
 })();
\ No newline at end of file

```