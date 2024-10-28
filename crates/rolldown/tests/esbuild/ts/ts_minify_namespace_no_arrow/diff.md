# Diff
## /a.js
### esbuild
```js
var Foo;(function(e){let a;(function(p){foo(e,p)})(a=e.Bar||={})})(Foo||={});
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
@@ -1,7 +1,7 @@
 var Foo;
-(function (e) {
-    let a;
-    (function (p) {
-        foo(e, p);
-    })(a = e.Bar ||= {});
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
export var Foo;(function(e){let a;(function(p){foo(e,p)})(a=e.Bar||={})})(Foo||={});
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
@@ -1,7 +1,8 @@
-export var Foo;
-(function (e) {
-    let a;
-    (function (p) {
-        foo(e, p);
-    })(a = e.Bar ||= {});
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