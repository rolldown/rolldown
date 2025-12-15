## /out.js
### esbuild
```js
// entry.js
var n0 = Symbol({});
var n1 = Symbol(/./);
var n2 = Symbol(() => 0);
var n3 = Symbol(x);
var n4 = new Symbol("abc");
var n5 = Symbol(1, 2, 3);
var n6 = /* @__PURE__ */ Symbol((() => Math.random() < 0.5)() ? "x" : "y");
```
### rolldown
```js
//#region entry.js
Symbol(x);
Symbol((() => Math.random() < .5)() ? "x" : "y");

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,2 @@
-var n0 = Symbol({});
-var n1 = Symbol(/./);
-var n2 = Symbol(() => 0);
-var n3 = Symbol(x);
-var n4 = new Symbol("abc");
-var n5 = Symbol(1, 2, 3);
-var n6 = Symbol((() => Math.random() < 0.5)() ? "x" : "y");
+Symbol(x);
+Symbol((() => Math.random() < .5)() ? "x" : "y");

```