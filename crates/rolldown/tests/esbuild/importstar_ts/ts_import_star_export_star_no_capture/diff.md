# Diff
## /out.js
### esbuild
```js
// foo.ts
var foo = 123;

// entry.ts
var foo2 = 234;
console.log(foo, foo, foo2);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,3 +0,0 @@
-var foo = 123;
-var foo2 = 234;
-console.log(foo, foo, foo2);

```