# Diff
## /out.js
### esbuild
```js
const keepThis = Symbol("keepThis");
keepThis;
class Foo {
}
(() => new Foo())();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,4 +0,0 @@
-const keepThis = Symbol("keepThis");
-keepThis;
-class Foo {}
-(() => new Foo())();

```