# Reason
1. generate same output as esbuild when `bundle` mode
# Diff
## /out.js
### esbuild
```js
"use strict";"use loose";a,b;
```
### rolldown
```js
'use loose'

//#region entry.js
a;
b;

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +1,3 @@
-"use strict";
-"use loose";
-(a, b);
+'use loose';
+a;
+b;

```