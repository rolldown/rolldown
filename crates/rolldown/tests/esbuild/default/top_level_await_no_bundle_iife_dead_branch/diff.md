# Reason
1. Strip `await` when format don't support top level await
# Diff
## /out.js
### esbuild
```js
(() => {
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