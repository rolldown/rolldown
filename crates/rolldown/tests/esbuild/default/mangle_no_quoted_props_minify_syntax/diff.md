# Diff
## /out/entry.js
### esbuild
```js
x._doNotMangleThis, x?._doNotMangleThis, x[y ? "_doNotMangleThis" : z], x?.[y ? "_doNotMangleThis" : z], x[y ? z : "_doNotMangleThis"], x?.[y ? z : "_doNotMangleThis"];
var { _doNotMangleThis: x } = y;
"_doNotMangleThis" in x, (y ? "_doNotMangleThis" : z) in x, (y ? z : "_doNotMangleThis") in x;
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,3 +0,0 @@
-(x._doNotMangleThis, x?._doNotMangleThis, x[y ? "_doNotMangleThis" : z], x?.[y ? "_doNotMangleThis" : z], x[y ? z : "_doNotMangleThis"], x?.[y ? z : "_doNotMangleThis"]);
-var {_doNotMangleThis: x} = y;
-(("_doNotMangleThis" in x), ((y ? "_doNotMangleThis" : z) in x), ((y ? z : "_doNotMangleThis") in x));

```