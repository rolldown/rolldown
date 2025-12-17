## /out.js
### esbuild
```js
// entry.ts
var a = foo.a;
var b = a.b;
var c = b.c;
var bar = c;
export {
  bar
};
```
### rolldown
```js
//#region entry.ts
var c = foo.a.b.c;
foo.x.y;
let bar = c;

//#endregion
export { bar };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,5 +1,4 @@
-var a = foo.a;
-var b = a.b;
-var c = b.c;
+var c = foo.a.b.c;
+foo.x.y;
 var bar = c;
 export {bar};

```