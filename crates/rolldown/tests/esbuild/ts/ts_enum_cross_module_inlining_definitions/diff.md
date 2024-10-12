# Diff
## /out/entry.js
### esbuild
```js
// enums.ts
var a = ((a2) => {
  a2[a2["implicit_number"] = 0] = "implicit_number";
  a2[a2["explicit_number"] = 123] = "explicit_number";
  a2["explicit_string"] = "xyz";
  a2[a2["non_constant"] = foo] = "non_constant";
  return a2;
})(a || {});

// entry.ts
console.log([
  0 /* implicit_number */,
  123 /* explicit_number */,
  "xyz" /* explicit_string */,
  a.non_constant
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
@@ -1,8 +0,0 @@
-var a = (a2 => {
-    a2[a2["implicit_number"] = 0] = "implicit_number";
-    a2[a2["explicit_number"] = 123] = "explicit_number";
-    a2["explicit_string"] = "xyz";
-    a2[a2["non_constant"] = foo] = "non_constant";
-    return a2;
-})(a || ({}));
-console.log([0, 123, "xyz", a.non_constant]);

```