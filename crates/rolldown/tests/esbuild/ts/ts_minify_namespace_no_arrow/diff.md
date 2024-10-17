# Diff
## /a.js
### esbuild
```js
var Foo;(function(e){let a;(function(p){foo(e,p)})(a=e.Bar||={})})(Foo||={});
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/a.js
+++ rolldown	
@@ -1,7 +0,0 @@
-var Foo;
-(function (e) {
-    let a;
-    (function (p) {
-        foo(e, p);
-    })(a = e.Bar ||= {});
-})(Foo ||= {});

```
## /b.js
### esbuild
```js
export var Foo;(function(e){let a;(function(p){foo(e,p)})(a=e.Bar||={})})(Foo||={});
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/b.js
+++ rolldown	
@@ -1,7 +0,0 @@
-export var Foo;
-(function (e) {
-    let a;
-    (function (p) {
-        foo(e, p);
-    })(a = e.Bar ||= {});
-})(Foo ||= {});

```