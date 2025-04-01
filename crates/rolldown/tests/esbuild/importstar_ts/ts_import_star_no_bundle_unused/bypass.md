# Reason
1. top level var rewrite
# Diff
## /out.js
### esbuild
```js
let foo = 234;
console.log(foo);
```
### rolldown
```js

//#region entry.ts
let foo = 234;
console.log(foo);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,2 +1,2 @@
-let foo = 234;
+var foo = 234;
 console.log(foo);

```