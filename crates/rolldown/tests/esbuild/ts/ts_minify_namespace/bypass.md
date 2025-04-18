# Reason
1. could be done in minifier
# Diff
## /a.js
### esbuild
```js
var Foo;(e=>{let a;(p=>foo(e,p))(a=e.Bar||={})})(Foo||={});
```
### rolldown
```js
//#region a.ts
let Foo;
(function(_Foo) {
	let Bar;
	(function(_Bar) {
		foo(Foo, Bar);
	})(Bar || (Bar = _Foo.Bar || (_Foo.Bar = {})));
})(Foo || (Foo = {}));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/a.js
+++ rolldown	a.js
@@ -1,5 +1,7 @@
 var Foo;
-(e => {
-    let a;
-    (p => foo(e, p))(a = e.Bar ||= {});
-})(Foo ||= {});
+(function (_Foo) {
+    let Bar;
+    (function (_Bar) {
+        foo(Foo, Bar);
+    })(Bar || (Bar = _Foo.Bar || (_Foo.Bar = {})));
+})(Foo || (Foo = {}));

```
## /b.js
### esbuild
```js
export var Foo;(e=>{let a;(p=>foo(e,p))(a=e.Bar||={})})(Foo||={});
```
### rolldown
```js
//#region b.ts
let Foo;
(function(_Foo) {
	let Bar;
	(function(_Bar) {
		foo(Foo, Bar);
	})(Bar || (Bar = _Foo.Bar || (_Foo.Bar = {})));
})(Foo || (Foo = {}));

//#endregion
export { Foo };
```
### diff
```diff
===================================================================
--- esbuild	/b.js
+++ rolldown	b.js
@@ -1,5 +1,8 @@
-export var Foo;
-(e => {
-    let a;
-    (p => foo(e, p))(a = e.Bar ||= {});
-})(Foo ||= {});
+var Foo;
+(function (_Foo) {
+    let Bar;
+    (function (_Bar) {
+        foo(Foo, Bar);
+    })(Bar || (Bar = _Foo.Bar || (_Foo.Bar = {})));
+})(Foo || (Foo = {}));
+export {Foo};

```