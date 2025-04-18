# Reason
1. same output as esbuild in `bundle` mode https://hyrious.me/esbuild-repl/?version=0.23.0&b=e%00entry.js%00x%5B%27_doNotMangleThis%27%5D%3B%0Ax%3F.%5B%27_doNotMangleThis%27%5D%3B%0Ax%5By+%3F+%27_doNotMangleThis%27+%3A+z%5D%3B%0Ax%3F.%5By+%3F+%27_doNotMangleThis%27+%3A+z%5D%3B%0Ax%5By+%3F+z+%3A+%27_doNotMangleThis%27%5D%3B%0Ax%3F.%5By+%3F+z+%3A+%27_doNotMangleThis%27%5D%3B%0A%28%7B+%27_doNotMangleThis%27%3A+x+%7D%29%3B%0A%28class+%7B+%27_doNotMangleThis%27+%3D+x+%7D%29%3B%0Avar+%7B+%27_doNotMangleThis%27%3A+x+%7D+%3D+y%3B%0A%27_doNotMangleThis%27+in+x%3B%0A%28y+%3F+%27_doNotMangleThis%27+%3A+z%29+in+x%3B%0A%28y+%3F+z+%3A+%27_doNotMangleThis%27%29+in+x%3B%0A&b=%00file.js%00&o=%7B%0A++treeShaking%3A+true%2C%0A++external%3A+%5B%22c%22%2C+%22a%22%2C+%22b%22%5D%2C%0A%22bundle%22%3A+true%2C%0Aformat%3A+%22esm%22%0A%7D
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