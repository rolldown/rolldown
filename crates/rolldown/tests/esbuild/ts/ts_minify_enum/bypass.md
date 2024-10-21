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

//#region a.ts
var Foo = function(Foo$1) {
	Foo$1[Foo$1["A"] = 0] = "A";
	Foo$1[Foo$1["B"] = 1] = "B";
	Foo$1[Foo$1["C"] = Foo$1] = "C";
	return Foo$1;
}(Foo || {});

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/a.js
+++ rolldown	a.js
@@ -1,1 +1,6 @@
-var Foo = (e => (e[e.A = 0] = "A", e[e.B = 1] = "B", e[e.C = e] = "C", e))(Foo || ({}));
+var Foo = (function (Foo$1) {
+    Foo$1[Foo$1["A"] = 0] = "A";
+    Foo$1[Foo$1["B"] = 1] = "B";
+    Foo$1[Foo$1["C"] = Foo$1] = "C";
+    return Foo$1;
+})(Foo || ({}));

```
## /b.js
### esbuild
```js
export var Foo=(e=>(e[e.X=0]="X",e[e.Y=1]="Y",e[e.Z=e]="Z",e))(Foo||{});
```
### rolldown
```js

//#region b.ts
let Foo = function(Foo$1) {
	Foo$1[Foo$1["X"] = 0] = "X";
	Foo$1[Foo$1["Y"] = 1] = "Y";
	Foo$1[Foo$1["Z"] = Foo$1] = "Z";
	return Foo$1;
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
+var Foo = (function (Foo$1) {
+    Foo$1[Foo$1["X"] = 0] = "X";
+    Foo$1[Foo$1["Y"] = 1] = "Y";
+    Foo$1[Foo$1["Z"] = Foo$1] = "Z";
+    return Foo$1;
+})({});
+export {Foo};

```