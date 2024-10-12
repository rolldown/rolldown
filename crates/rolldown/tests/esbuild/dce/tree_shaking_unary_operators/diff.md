# Diff
## /out.js
### esbuild
```js
(() => {
  // entry.js
  var keep;
  +keep;
  -keep;
  ~keep;
  delete keep;
  ++keep;
  --keep;
  keep++;
  keep--;
})();
```
### rolldown
```js

//#region entry.js
let keep;
++keep;
--keep;
keep++;
keep--;

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,12 +1,9 @@
-(() => {
-  // entry.js
-  var keep;
-  +keep;
-  -keep;
-  ~keep;
-  delete keep;
-  ++keep;
-  --keep;
-  keep++;
-  keep--;
-})();
\ No newline at end of file
+
+//#region entry.js
+let keep;
+++keep;
+--keep;
+keep++;
+keep--;
+
+//#endregion

```