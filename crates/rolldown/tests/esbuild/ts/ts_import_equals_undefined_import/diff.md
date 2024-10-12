# Diff
## /out.js
### esbuild
```js
// import.ts
var value = 123;

// entry.ts
var value_copy = value;
var foo = value_copy;
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
@@ -1,4 +0,0 @@
-var value = 123;
-var value_copy = value;
-var foo = value_copy;
-console.log(foo);

```