# Reason
1. the `console` was revemod due to oxc transformer which is expected
# Diff
## /out.js
### esbuild
```js
(() => {
  // entry.js
  if (false) console.log(hasBar);
})();
```
### rolldown
```js
(function() {


})();
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +1,1 @@
-(() => {
-    if (false) console.log(hasBar);
-})();
+(function () {})();

```