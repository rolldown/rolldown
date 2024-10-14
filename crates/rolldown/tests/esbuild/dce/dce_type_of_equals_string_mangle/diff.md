# Reason
1. side effects detect
# Diff
## /out.js
### esbuild
```js
(() => {
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
@@ -1,1 +1,3 @@
-(() => {})();
+(function () {
+    var hasBar = typeof bar !== "undefined";
+})();

```
