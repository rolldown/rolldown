# Reason
1. wrong tree shaking result
# Diff
## /out/entry.js
### esbuild
```js
x["_doNotMangleThis"];
x?.["_doNotMangleThis"];
x[y ? "_doNotMangleThis" : z];
x?.[y ? "_doNotMangleThis" : z];
x[y ? z : "_doNotMangleThis"];
x?.[y ? z : "_doNotMangleThis"];
({ "_doNotMangleThis": x });
(class {
  "_doNotMangleThis" = x;
});
var { "_doNotMangleThis": x } = y;
"_doNotMangleThis" in x;
(y ? "_doNotMangleThis" : z) in x;
(y ? z : "_doNotMangleThis") in x;
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
@@ -3,14 +3,8 @@
 x[y ? "_doNotMangleThis" : z];
 x?.[y ? "_doNotMangleThis" : z];
 x[y ? z : "_doNotMangleThis"];
 x?.[y ? z : "_doNotMangleThis"];
-({
-    "_doNotMangleThis": x
-});
-(class {
-    "_doNotMangleThis" = x;
-});
 var {"_doNotMangleThis": x} = y;
 ("_doNotMangleThis" in x);
 ((y ? "_doNotMangleThis" : z) in x);
 ((y ? z : "_doNotMangleThis") in x);

```