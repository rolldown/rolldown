# Reason
1. side effects detect
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


//#region entry.js
var hasBar = typeof bar !== "undefined";

//#endregion
})();

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +1,3 @@
-(() => {
-    if (false) console.log(hasBar);
+(function () {
+    var hasBar = typeof bar !== "undefined";
 })();

```