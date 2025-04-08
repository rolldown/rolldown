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
let a_DROP = /* @__PURE__ */ function(a_DROP) {
	a_DROP[a_DROP["x"] = 1] = "x";
	return a_DROP;
}({});
let b_DROP = /* @__PURE__ */ function(b_DROP) {
	b_DROP[b_DROP["x"] = 2] = "x";
	return b_DROP;
}({});
let c_DROP = /* @__PURE__ */ function(c_DROP) {
	c_DROP["x"] = "";
	return c_DROP;
}({});
let a_keep = /* @__PURE__ */ function(a_keep) {
	a_keep[a_keep["x"] = false] = "x";
	return a_keep;
}({});
let b_keep = /* @__PURE__ */ function(b_keep) {
	b_keep[b_keep["x"] = foo] = "x";
	return b_keep;
}({});
let c_keep = /* @__PURE__ */ function(c_keep) {
	c_keep[c_keep["x"] = 3] = "x";
	return c_keep;
}({});
let d_keep = /* @__PURE__ */ function(d_keep) {
	d_keep[d_keep["x"] = 4] = "x";
	return d_keep;
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
+var a_DROP = (function (a_DROP) {
+    a_DROP[a_DROP["x"] = 1] = "x";
+    return a_DROP;
+})({});
+var b_DROP = (function (b_DROP) {
+    b_DROP[b_DROP["x"] = 2] = "x";
+    return b_DROP;
+})({});
+var c_DROP = (function (c_DROP) {
+    c_DROP["x"] = "";
+    return c_DROP;
+})({});
+var a_keep = (function (a_keep) {
+    a_keep[a_keep["x"] = false] = "x";
+    return a_keep;
+})({});
+var b_keep = (function (b_keep) {
+    b_keep[b_keep["x"] = foo] = "x";
+    return b_keep;
+})({});
+var c_keep = (function (c_keep) {
+    c_keep[c_keep["x"] = 3] = "x";
+    return c_keep;
+})({});
+var d_keep = (function (d_keep) {
+    d_keep[d_keep["x"] = 4] = "x";
+    return d_keep;
+})({});
 var e_keep = {};
-console.log([1, 2, ""]);
+console.log([a_DROP.x, b_DROP["x"], c_DROP.x]);
 console.log([a_keep.x, b_keep.x, c_keep, d_keep.y, e_keep.x]);

```