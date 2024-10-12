# Diff
## /out.js
### esbuild
```js
function o(){return 123}console.log(o());
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,4 +0,0 @@
-function o() {
-    return 123;
-}
-console.log(o());

```