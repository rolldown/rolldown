# Diff
## /a.js
### esbuild
```js
var Foo;(e=>{let a;(p=>foo(e,p))(a=e.Bar||={})})(Foo||={});
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/a.js
+++ rolldown	
@@ -1,5 +0,0 @@
-var Foo;
-(e => {
-    let a;
-    (p => foo(e, p))(a = e.Bar ||= {});
-})(Foo ||= {});

```
## /b.js
### esbuild
```js
export var Foo;(e=>{let a;(p=>foo(e,p))(a=e.Bar||={})})(Foo||={});
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/b.js
+++ rolldown	
@@ -1,5 +0,0 @@
-export var Foo;
-(e => {
-    let a;
-    (p => foo(e, p))(a = e.Bar ||= {});
-})(Foo ||= {});

```