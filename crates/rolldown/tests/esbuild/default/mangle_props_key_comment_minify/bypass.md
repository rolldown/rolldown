# Reason
1. could be done in minifier
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

//#region entry.js
x = class {
	_mangleThis = 1;
	["_mangleThisToo"] = 2;
	"_doNotMangleThis" = 3;
};
x = {
	_mangleThis: 1,
	["_mangleThisToo"]: 2,
	"_doNotMangleThis": 3
};
x._mangleThis = 1;
x["_mangleThisToo"] = 2;
x["_doNotMangleThis"] = 3;
x([`${foo}._mangleThis = bar._mangleThisToo`, `${foo}.notMangled = bar.notMangledEither`]);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,9 +1,14 @@
-(x = class {
-    a = 1;
-    b = 2;
-    _doNotMangleThis = 3;
-}, x = {
-    a: 1,
-    b: 2,
-    _doNotMangleThis: 3
-}, x.a = 1, x.b = 2, x._doNotMangleThis = 3, x([`${foo}.a = bar.b`, `${foo}.notMangled = bar.notMangledEither`]));
+x = class {
+    _mangleThis = 1;
+    ["_mangleThisToo"] = 2;
+    "_doNotMangleThis" = 3;
+};
+x = {
+    _mangleThis: 1,
+    ["_mangleThisToo"]: 2,
+    "_doNotMangleThis": 3
+};
+x._mangleThis = 1;
+x["_mangleThisToo"] = 2;
+x["_doNotMangleThis"] = 3;
+x([`${foo}._mangleThis = bar._mangleThisToo`, `${foo}.notMangled = bar.notMangledEither`]);

```