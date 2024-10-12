# Diff
## /out/entry.js
### esbuild
```js
// entry.jsx
import "foo" assert { type: "json" };
import "foo" assert { type: "json" };
import "foo" assert {
  /* before */
  type: "json"
};
import "foo" assert {
  type:
    /* before */
    "json"
};
import "foo" assert {
  type: "json"
  /* before */
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,16 +0,0 @@
-// entry.jsx
-import "foo" assert { type: "json" };
-import "foo" assert { type: "json" };
-import "foo" assert {
-  /* before */
-  type: "json"
-};
-import "foo" assert {
-  type:
-    /* before */
-    "json"
-};
-import "foo" assert {
-  type: "json"
-  /* before */
-};
\ No newline at end of file

```