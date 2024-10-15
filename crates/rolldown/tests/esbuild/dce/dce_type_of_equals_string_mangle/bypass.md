# Reason
1. different iife wrapper, trivial diff
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


})();
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,1 +1,1 @@
-(() => {})();
+(function () {})();

```