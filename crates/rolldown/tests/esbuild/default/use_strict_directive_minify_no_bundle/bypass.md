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

//#region entry.js
"use loose";
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
 "use loose";
-(a, b);
+a;
+b;

```