# Diff
## /out/entry.js
### esbuild
```js
// entry.ts
var c_num = /* @__PURE__ */ ((c_num2) => {
  c_num2[c_num2["x"] = 123] = "x";
  return c_num2;
})(c_num || {});
var d_num = /* @__PURE__ */ ((d_num2) => {
  d_num2[d_num2["x"] = 123] = "x";
  return d_num2;
})(d_num || {});
var e_num = /* @__PURE__ */ ((e_num2) => {
  e_num2[e_num2["x"] = 123] = "x";
  return e_num2;
})(e_num || {});
var c_str = /* @__PURE__ */ ((c_str2) => {
  c_str2["x"] = "abc";
  return c_str2;
})(c_str || {});
var d_str = /* @__PURE__ */ ((d_str2) => {
  d_str2["x"] = "abc";
  return d_str2;
})(d_str || {});
var e_str = /* @__PURE__ */ ((e_str2) => {
  e_str2["x"] = "abc";
  return e_str2;
})(e_str || {});
inlined = [
  123 /* x */,
  123 /* x */,
  "abc" /* x */,
  "abc" /* x */
];
not_inlined = [
  c_num?.x,
  d_num?.["x"],
  e_num,
  c_str?.x,
  d_str?.["x"],
  e_str
];
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,26 +0,0 @@
-var c_num = (c_num2 => {
-    c_num2[c_num2["x"] = 123] = "x";
-    return c_num2;
-})(c_num || ({}));
-var d_num = (d_num2 => {
-    d_num2[d_num2["x"] = 123] = "x";
-    return d_num2;
-})(d_num || ({}));
-var e_num = (e_num2 => {
-    e_num2[e_num2["x"] = 123] = "x";
-    return e_num2;
-})(e_num || ({}));
-var c_str = (c_str2 => {
-    c_str2["x"] = "abc";
-    return c_str2;
-})(c_str || ({}));
-var d_str = (d_str2 => {
-    d_str2["x"] = "abc";
-    return d_str2;
-})(d_str || ({}));
-var e_str = (e_str2 => {
-    e_str2["x"] = "abc";
-    return e_str2;
-})(e_str || ({}));
-inlined = [123, 123, "abc", "abc"];
-not_inlined = [c_num?.x, d_num?.["x"], e_num, c_str?.x, d_str?.["x"], e_str];

```