# Diff
## /out/entry.js
### esbuild
```js
// entry.js
//! <script>foo<\/script>
var x;
export {
  x
};
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