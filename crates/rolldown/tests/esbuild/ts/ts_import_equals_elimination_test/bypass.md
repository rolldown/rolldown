# Reason
1. rolldown is not ts aware, it's not possibly support for now
2. sub optimal
# Diff
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
var a = foo.a;
var b = a.b;
var c = b.c;
var x = foo.x;
var y = x.y;
let bar = c;
//#endregion

export { bar };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,5 +1,7 @@
 var a = foo.a;
 var b = a.b;
 var c = b.c;
+var x = foo.x;
+var y = x.y;
 var bar = c;
 export {bar};

```