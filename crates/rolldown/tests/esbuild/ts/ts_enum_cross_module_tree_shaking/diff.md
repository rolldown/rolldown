# Diff
## /out/entry.js
### esbuild
```js
// enums.ts
var a_keep = /* @__PURE__ */ ((a_keep2) => {
  a_keep2[a_keep2["x"] = false] = "x";
  return a_keep2;
})(a_keep || {});
var b_keep = ((b_keep2) => {
  b_keep2[b_keep2["x"] = foo] = "x";
  return b_keep2;
})(b_keep || {});
var c_keep = /* @__PURE__ */ ((c_keep2) => {
  c_keep2[c_keep2["x"] = 3] = "x";
  return c_keep2;
})(c_keep || {});
var d_keep = /* @__PURE__ */ ((d_keep2) => {
  d_keep2[d_keep2["x"] = 4] = "x";
  return d_keep2;
})(d_keep || {});
var e_keep = {};

// entry.ts
console.log([
  1 /* x */,
  2 /* x */,
  "" /* x */
]);
console.log([
  a_keep.x,
  b_keep.x,
  c_keep,
  d_keep.y,
  e_keep.x
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
@@ -1,19 +0,0 @@
-var a_keep = (a_keep2 => {
-    a_keep2[a_keep2["x"] = false] = "x";
-    return a_keep2;
-})(a_keep || ({}));
-var b_keep = (b_keep2 => {
-    b_keep2[b_keep2["x"] = foo] = "x";
-    return b_keep2;
-})(b_keep || ({}));
-var c_keep = (c_keep2 => {
-    c_keep2[c_keep2["x"] = 3] = "x";
-    return c_keep2;
-})(c_keep || ({}));
-var d_keep = (d_keep2 => {
-    d_keep2[d_keep2["x"] = 4] = "x";
-    return d_keep2;
-})(d_keep || ({}));
-var e_keep = {};
-console.log([1, 2, ""]);
-console.log([a_keep.x, b_keep.x, c_keep, d_keep.y, e_keep.x]);

```