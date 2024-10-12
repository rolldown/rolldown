# Diff
## /out.js
### esbuild
```js
const keepThisToo = Symbol("keepThisToo");
class Foo {
  keepThis;
  [keepThisToo];
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
@@ -1,6 +0,0 @@
-const keepThisToo = Symbol("keepThisToo");
-class Foo {
-    keepThis;
-    [keepThisToo];
-}
-(() => new Foo())();

```