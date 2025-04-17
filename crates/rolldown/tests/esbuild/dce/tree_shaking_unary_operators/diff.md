# Reason
1. unary operator side effects
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
(function() {


//#region entry.js
let keep;
++keep;
--keep;
keep++;
keep--;

})();
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,12 +1,11 @@
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
+(function() {
+
+
+//#region entry.js
+let keep;
+++keep;
+--keep;
+keep++;
+keep--;
+
 })();
\ No newline at end of file

```