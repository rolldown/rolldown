# Diff
## /out.js
### esbuild
```js
// foo.ts
var foo_exports = {};

// entry.js
console.log(foo_exports);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var foo_exports = {};
-console.log(foo_exports);

```