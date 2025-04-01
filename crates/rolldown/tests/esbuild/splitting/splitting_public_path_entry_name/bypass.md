# Reason
1. different chunk naming style
# Diff
## /out/a.js
### esbuild
```js
// a.js
import("/www/b-AQIID5BE.js");
```
### rolldown
```js

//#region a.js
import("./b.js");
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	a.js
@@ -1,1 +1,1 @@
-import("/www/b-AQIID5BE.js");
+import("./b.js");

```