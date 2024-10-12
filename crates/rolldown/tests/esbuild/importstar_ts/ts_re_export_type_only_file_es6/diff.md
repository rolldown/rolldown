# Diff
## /out.js
### esbuild
```js
// types1.ts
console.log("some code");

// types2.ts
console.log("some code");

// types3.ts
console.log("some code");

// values.ts
var foo = 123;

// entry.ts
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
@@ -1,5 +0,0 @@
-console.log("some code");
-console.log("some code");
-console.log("some code");
-var foo = 123;
-console.log(foo);

```