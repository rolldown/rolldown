# Reason
1. enum side effects
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

//#region enums.ts
let a_DROP = /* @__PURE__ */ function(a_DROP$1) {
	a_DROP$1[a_DROP$1["x"] = 1] = "x";
	return a_DROP$1;
}({});
let b_DROP = /* @__PURE__ */ function(b_DROP$1) {
	b_DROP$1[b_DROP$1["x"] = 2] = "x";
	return b_DROP$1;
}({});
let c_DROP = /* @__PURE__ */ function(c_DROP$1) {
	c_DROP$1["x"] = "";
	return c_DROP$1;
}({});
let a_keep = /* @__PURE__ */ function(a_keep$1) {
	a_keep$1[a_keep$1["x"] = false] = "x";
	return a_keep$1;
}({});
let b_keep = /* @__PURE__ */ function(b_keep$1) {
	b_keep$1[b_keep$1["x"] = foo] = "x";
	return b_keep$1;
}({});
let c_keep = /* @__PURE__ */ function(c_keep$1) {
	c_keep$1[c_keep$1["x"] = 3] = "x";
	return c_keep$1;
}({});
let d_keep = /* @__PURE__ */ function(d_keep$1) {
	d_keep$1[d_keep$1["x"] = 4] = "x";
	return d_keep$1;
}({});
let e_keep = {};
//#endregion

//#region entry.ts
console.log([
	a_DROP.x,
	b_DROP["x"],
	c_DROP.x
]);
console.log([
	a_keep.x,
	b_keep.x,
	c_keep,
	d_keep.y,
	e_keep.x
]);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,19 +1,31 @@
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
+var a_DROP = (function (a_DROP$1) {
+    a_DROP$1[a_DROP$1["x"] = 1] = "x";
+    return a_DROP$1;
+})({});
+var b_DROP = (function (b_DROP$1) {
+    b_DROP$1[b_DROP$1["x"] = 2] = "x";
+    return b_DROP$1;
+})({});
+var c_DROP = (function (c_DROP$1) {
+    c_DROP$1["x"] = "";
+    return c_DROP$1;
+})({});
+var a_keep = (function (a_keep$1) {
+    a_keep$1[a_keep$1["x"] = false] = "x";
+    return a_keep$1;
+})({});
+var b_keep = (function (b_keep$1) {
+    b_keep$1[b_keep$1["x"] = foo] = "x";
+    return b_keep$1;
+})({});
+var c_keep = (function (c_keep$1) {
+    c_keep$1[c_keep$1["x"] = 3] = "x";
+    return c_keep$1;
+})({});
+var d_keep = (function (d_keep$1) {
+    d_keep$1[d_keep$1["x"] = 4] = "x";
+    return d_keep$1;
+})({});
 var e_keep = {};
-console.log([1, 2, ""]);
+console.log([a_DROP.x, b_DROP["x"], c_DROP.x]);
 console.log([a_keep.x, b_keep.x, c_keep, d_keep.y, e_keep.x]);

```