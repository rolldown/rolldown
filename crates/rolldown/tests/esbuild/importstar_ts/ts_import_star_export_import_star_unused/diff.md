# Diff
## /out.js
### esbuild
```js
// entry.ts
var foo = 234;
console.log(foo);
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
-var foo = 234;
-console.log(foo);

```