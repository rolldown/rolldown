# Reason
1. Could be done in minifier
# Diff
## /a.js
### esbuild
```js
var Foo=(e=>(e[e.A=0]="A",e[e.B=1]="B",e[e.C=e]="C",e))(Foo||{});
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/a.js
+++ rolldown	a.js
@@ -1,1 +0,0 @@
-var Foo = (e => (e[e.A = 0] = "A", e[e.B = 1] = "B", e[e.C = e] = "C", e))(Foo || ({}));

```
## /b.js
### esbuild
```js
export var Foo=(e=>(e[e.X=0]="X",e[e.Y=1]="Y",e[e.Z=e]="Z",e))(Foo||{});
```
### rolldown
```js

//#region b.ts
let Foo = /* @__PURE__ */ function(Foo) {
	Foo[Foo["X"] = 0] = "X";
	Foo[Foo["Y"] = 1] = "Y";
	Foo[Foo["Z"] = Foo] = "Z";
	return Foo;
}({});

//#endregion
export { Foo };
```
### diff
```diff
===================================================================
--- esbuild	/b.js
+++ rolldown	b.js
@@ -1,1 +1,7 @@
-export var Foo = (e => (e[e.X = 0] = "X", e[e.Y = 1] = "Y", e[e.Z = e] = "Z", e))(Foo || ({}));
+var Foo = (function (Foo) {
+    Foo[Foo["X"] = 0] = "X";
+    Foo[Foo["Y"] = 1] = "Y";
+    Foo[Foo["Z"] = Foo] = "Z";
+    return Foo;
+})({});
+export {Foo};

```