# Reason
1. needs in IIFE pure annotation
# Diff
## /out/entry.js
### esbuild
```js
var a = /* @__PURE__ */ ((a2) => {
  a2[a2["b"] = 123] = "b";
  a2[a2["c"] = 123 /* b */] = "c";
  return a2;
})(a || {});
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,5 +0,0 @@
-var a = (a2 => {
-    a2[a2["b"] = 123] = "b";
-    a2[a2["c"] = 123] = "c";
-    return a2;
-})(a || ({}));

```