# Reason
1. could be done in minifier
# Diff
## /out/mangle.js
### esbuild
```js
x.a;
x?.a;
x[y ? "a" : z];
x?.[y ? "a" : z];
x[y ? z : "a"];
x?.[y ? z : "a"];
x[y, "a"];
x?.[y, "a"];
({ a: x });
({ ["a"]: x });
({ [(y, "a")]: x });
(class {
  a = x;
});
(class {
  ["a"] = x;
});
(class {
  [(y, "a")] = x;
});
var { a: x } = y;
var { ["a"]: x } = y;
var { [(z, "a")]: x } = y;
"a" in x;
(y ? "a" : z) in x;
(y ? z : "a") in x;
(y, "a") in x;
```
### rolldown
```js

//#region mangle.js
x["_mangleThis"];
x?.["_mangleThis"];
x[y ? "_mangleThis" : z];
x?.[y ? "_mangleThis" : z];
x[y ? z : "_mangleThis"];
x?.[y ? z : "_mangleThis"];
x[y, "_mangleThis"];
x?.[y, "_mangleThis"];
({ [(y, "_mangleThis")]: x });
(class {
	[(y, "_mangleThis")] = x;
});
var { "_mangleThis": x } = y;
var { ["_mangleThis"]: x } = y;
var { [(z, "_mangleThis")]: x } = y;
"_mangleThis" in x;
(y ? "_mangleThis" : z) in x;
(y ? z : "_mangleThis") in x;
(y, "_mangleThis") in x;

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/mangle.js
+++ rolldown	mangle.js
@@ -1,33 +1,21 @@
-x.a;
-x?.a;
-x[y ? "a" : z];
-x?.[y ? "a" : z];
-x[y ? z : "a"];
-x?.[y ? z : "a"];
-x[(y, "a")];
-x?.[(y, "a")];
+x["_mangleThis"];
+x?.["_mangleThis"];
+x[y ? "_mangleThis" : z];
+x?.[y ? "_mangleThis" : z];
+x[y ? z : "_mangleThis"];
+x?.[y ? z : "_mangleThis"];
+x[(y, "_mangleThis")];
+x?.[(y, "_mangleThis")];
 ({
-    a: x
+    [(y, "_mangleThis")]: x
 });
-({
-    ["a"]: x
-});
-({
-    [(y, "a")]: x
-});
 (class {
-    a = x;
+    [(y, "_mangleThis")] = x;
 });
-(class {
-    ["a"] = x;
-});
-(class {
-    [(y, "a")] = x;
-});
-var {a: x} = y;
-var {["a"]: x} = y;
-var {[(z, "a")]: x} = y;
-("a" in x);
-((y ? "a" : z) in x);
-((y ? z : "a") in x);
-((y, "a") in x);
+var {"_mangleThis": x} = y;
+var {["_mangleThis"]: x} = y;
+var {[(z, "_mangleThis")]: x} = y;
+("_mangleThis" in x);
+((y ? "_mangleThis" : z) in x);
+((y ? z : "_mangleThis") in x);
+((y, "_mangleThis") in x);

```