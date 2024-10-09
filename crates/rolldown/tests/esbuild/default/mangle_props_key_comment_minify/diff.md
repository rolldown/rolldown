# Diff
## /out/entry.js
### esbuild
```js
x = class {
  a = 1;
  b = 2;
  _doNotMangleThis = 3;
}, x = {
  a: 1,
  b: 2,
  _doNotMangleThis: 3
}, x.a = 1, x.b = 2, x._doNotMangleThis = 3, x([
  `${foo}.a = bar.b`,
  `${foo}.notMangled = bar.notMangledEither`
]);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,9 +0,0 @@
-(x = class {
-    a = 1;
-    b = 2;
-    _doNotMangleThis = 3;
-}, x = {
-    a: 1,
-    b: 2,
-    _doNotMangleThis: 3
-}, x.a = 1, x.b = 2, x._doNotMangleThis = 3, x([`${foo}.a = bar.b`, `${foo}.notMangled = bar.notMangledEither`]));

```