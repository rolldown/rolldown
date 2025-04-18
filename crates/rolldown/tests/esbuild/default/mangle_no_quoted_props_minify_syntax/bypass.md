# Reason
1. could be done in minifier
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
//#region entry.js
x["_doNotMangleThis"];
x?.["_doNotMangleThis"];
x[y ? "_doNotMangleThis" : z];
x?.[y ? "_doNotMangleThis" : z];
x[y ? z : "_doNotMangleThis"];
x?.[y ? z : "_doNotMangleThis"];
var { "_doNotMangleThis": x } = y;
"_doNotMangleThis" in x;
(y ? "_doNotMangleThis" : z) in x;
(y ? z : "_doNotMangleThis") in x;

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,3 +1,10 @@
-(x._doNotMangleThis, x?._doNotMangleThis, x[y ? "_doNotMangleThis" : z], x?.[y ? "_doNotMangleThis" : z], x[y ? z : "_doNotMangleThis"], x?.[y ? z : "_doNotMangleThis"]);
-var {_doNotMangleThis: x} = y;
-(("_doNotMangleThis" in x), ((y ? "_doNotMangleThis" : z) in x), ((y ? z : "_doNotMangleThis") in x));
+x["_doNotMangleThis"];
+x?.["_doNotMangleThis"];
+x[y ? "_doNotMangleThis" : z];
+x?.[y ? "_doNotMangleThis" : z];
+x[y ? z : "_doNotMangleThis"];
+x?.[y ? z : "_doNotMangleThis"];
+var {"_doNotMangleThis": x} = y;
+("_doNotMangleThis" in x);
+((y ? "_doNotMangleThis" : z) in x);
+((y ? z : "_doNotMangleThis") in x);

```