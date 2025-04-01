# Reason
1. different iife wrapper
# Diff
## /out.js
### esbuild
```js
(() => {
  function keep() {
  }
  keep();
})();
```
### rolldown
```js
(function() {


//#region entry.js
function keep() {}
keep();
//#endregion

})();
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,4 @@
-(() => {
+(function () {
     function keep() {}
     keep();
 })();

```