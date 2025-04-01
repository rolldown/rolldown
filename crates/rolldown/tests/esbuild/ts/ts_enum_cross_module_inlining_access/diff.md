# Reason
1. not support const enum inline
# Diff
## /out/entry.js
### esbuild
```js
// enums.ts
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

// entry.ts
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

//#region enums.ts
let a_num = /* @__PURE__ */ function(a_num$1) {
	a_num$1[a_num$1["x"] = 123] = "x";
	return a_num$1;
}({});
let b_num = /* @__PURE__ */ function(b_num$1) {
	b_num$1[b_num$1["x"] = 123] = "x";
	return b_num$1;
}({});
let c_num = /* @__PURE__ */ function(c_num$1) {
	c_num$1[c_num$1["x"] = 123] = "x";
	return c_num$1;
}({});
let d_num = /* @__PURE__ */ function(d_num$1) {
	d_num$1[d_num$1["x"] = 123] = "x";
	return d_num$1;
}({});
let e_num = /* @__PURE__ */ function(e_num$1) {
	e_num$1[e_num$1["x"] = 123] = "x";
	return e_num$1;
}({});
let a_str = /* @__PURE__ */ function(a_str$1) {
	a_str$1["x"] = "abc";
	return a_str$1;
}({});
let b_str = /* @__PURE__ */ function(b_str$1) {
	b_str$1["x"] = "abc";
	return b_str$1;
}({});
let c_str = /* @__PURE__ */ function(c_str$1) {
	c_str$1["x"] = "abc";
	return c_str$1;
}({});
let d_str = /* @__PURE__ */ function(d_str$1) {
	d_str$1["x"] = "abc";
	return d_str$1;
}({});
let e_str = /* @__PURE__ */ function(e_str$1) {
	e_str$1["x"] = "abc";
	return e_str$1;
}({});
//#endregion

//#region entry.ts
inlined = [
	a_num.x,
	b_num["x"],
	a_str.x,
	b_str["x"]
];
not_inlined = [
	c_num?.x,
	d_num?.["x"],
	e_num,
	c_str?.x,
	d_str?.["x"],
	e_str
];
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,26 +1,42 @@
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
+var a_num = (function (a_num$1) {
+    a_num$1[a_num$1["x"] = 123] = "x";
+    return a_num$1;
+})({});
+var b_num = (function (b_num$1) {
+    b_num$1[b_num$1["x"] = 123] = "x";
+    return b_num$1;
+})({});
+var c_num = (function (c_num$1) {
+    c_num$1[c_num$1["x"] = 123] = "x";
+    return c_num$1;
+})({});
+var d_num = (function (d_num$1) {
+    d_num$1[d_num$1["x"] = 123] = "x";
+    return d_num$1;
+})({});
+var e_num = (function (e_num$1) {
+    e_num$1[e_num$1["x"] = 123] = "x";
+    return e_num$1;
+})({});
+var a_str = (function (a_str$1) {
+    a_str$1["x"] = "abc";
+    return a_str$1;
+})({});
+var b_str = (function (b_str$1) {
+    b_str$1["x"] = "abc";
+    return b_str$1;
+})({});
+var c_str = (function (c_str$1) {
+    c_str$1["x"] = "abc";
+    return c_str$1;
+})({});
+var d_str = (function (d_str$1) {
+    d_str$1["x"] = "abc";
+    return d_str$1;
+})({});
+var e_str = (function (e_str$1) {
+    e_str$1["x"] = "abc";
+    return e_str$1;
+})({});
+inlined = [a_num.x, b_num["x"], a_str.x, b_str["x"]];
 not_inlined = [c_num?.x, d_num?.["x"], e_num, c_str?.x, d_str?.["x"], e_str];

```