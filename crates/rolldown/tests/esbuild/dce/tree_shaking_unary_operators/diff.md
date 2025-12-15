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

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,12 +0,0 @@
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

```