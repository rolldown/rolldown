# Diff
## /out/entry.js
### esbuild
```js
// entry.js
var x;
export {
  x
};
//! <script>foo<\/script>
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var x;
-export {x};

```