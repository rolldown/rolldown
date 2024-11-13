# Reason
1. this is expected, since we don't support `convertMode`
2. the diff is because oxc eliminated the dead branch
# Diff
## /out.js
### esbuild
```js
(() => {
  // entry.js
  if (false) foo;
  if (false) for (foo of bar) ;
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
@@ -1,4 +1,1 @@
-(() => {
-    if (false) foo;
-    if (false) for (foo of bar) ;
-})();
+(function () {})();

```