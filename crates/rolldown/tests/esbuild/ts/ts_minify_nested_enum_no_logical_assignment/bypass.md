# Reason
1. could be done in minifier
# Diff
## /a.js
### esbuild
```js
function foo(){let u;return(n=>(n[n.A=0]="A",n[n.B=1]="B",n[n.C=n]="C"))(u||(u={})),u}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/a.js
+++ rolldown	a.js
@@ -1,4 +0,0 @@
-function foo() {
-    let u;
-    return ((n => (n[n.A = 0] = "A", n[n.B = 1] = "B", n[n.C = n] = "C"))(u || (u = {})), u);
-}

```
## /b.js
### esbuild
```js
export function foo(){let e;return(n=>(n[n.X=0]="X",n[n.Y=1]="Y",n[n.Z=n]="Z"))(e||(e={})),e}
```
### rolldown
```js

//#region b.ts
function foo() {
	let Foo = /* @__PURE__ */ function(Foo$1) {
		Foo$1[Foo$1["X"] = 0] = "X";
		Foo$1[Foo$1["Y"] = 1] = "Y";
		Foo$1[Foo$1["Z"] = Foo$1] = "Z";
		return Foo$1;
	}({});
	return Foo;
}

export { foo };
```
### diff
```diff
===================================================================
--- esbuild	/b.js
+++ rolldown	b.js
@@ -1,4 +1,10 @@
-export function foo() {
-    let e;
-    return ((n => (n[n.X = 0] = "X", n[n.Y = 1] = "Y", n[n.Z = n] = "Z"))(e || (e = {})), e);
+function foo() {
+    let Foo = (function (Foo$1) {
+        Foo$1[Foo$1["X"] = 0] = "X";
+        Foo$1[Foo$1["Y"] = 1] = "Y";
+        Foo$1[Foo$1["Z"] = Foo$1] = "Z";
+        return Foo$1;
+    })({});
+    return Foo;
 }
+export {foo};

```