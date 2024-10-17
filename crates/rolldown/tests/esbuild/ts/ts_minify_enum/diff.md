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
+++ rolldown	
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

```
### diff
```diff
===================================================================
--- esbuild	/b.js
+++ rolldown	
@@ -1,1 +0,0 @@
-export var Foo = (e => (e[e.X = 0] = "X", e[e.Y = 1] = "Y", e[e.Z = e] = "Z", e))(Foo || ({}));

```