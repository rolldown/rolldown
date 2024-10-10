# Diff
## /out/keep.js
### esbuild
```js
foo("_keepThisProperty");
foo((x, "_keepThisProperty"));
foo(x ? "_keepThisProperty" : "_keepThisPropertyToo");
x[foo("_keepThisProperty")];
x?.[foo("_keepThisProperty")];
({ [foo("_keepThisProperty")]: x });
(class {
  [foo("_keepThisProperty")] = x;
});
var { [foo("_keepThisProperty")]: x } = y;
foo("_keepThisProperty") in x;
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/keep.js
+++ rolldown	
@@ -1,13 +0,0 @@
-foo("_keepThisProperty");
-foo((x, "_keepThisProperty"));
-foo(x ? "_keepThisProperty" : "_keepThisPropertyToo");
-x[foo("_keepThisProperty")];
-x?.[foo("_keepThisProperty")];
-({
-    [foo("_keepThisProperty")]: x
-});
-(class {
-    [foo("_keepThisProperty")] = x;
-});
-var {[foo("_keepThisProperty")]: x} = y;
-(foo("_keepThisProperty") in x);

```
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

```
### diff
```diff
===================================================================
--- esbuild	/out/mangle.js
+++ rolldown	
@@ -1,33 +0,0 @@
-x.a;
-x?.a;
-x[y ? "a" : z];
-x?.[y ? "a" : z];
-x[y ? z : "a"];
-x?.[y ? z : "a"];
-x[(y, "a")];
-x?.[(y, "a")];
-({
-    a: x
-});
-({
-    ["a"]: x
-});
-({
-    [(y, "a")]: x
-});
-(class {
-    a = x;
-});
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

```