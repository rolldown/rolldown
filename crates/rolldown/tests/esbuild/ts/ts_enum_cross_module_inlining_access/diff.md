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
let a_num = /* @__PURE__ */ function(a_num) {
	a_num[a_num["x"] = 123] = "x";
	return a_num;
}({});
let b_num$1 = /* @__PURE__ */ function(b_num) {
	b_num[b_num["x"] = 123] = "x";
	return b_num;
}({});
let c_num$1 = /* @__PURE__ */ function(c_num) {
	c_num[c_num["x"] = 123] = "x";
	return c_num;
}({});
let d_num$1 = /* @__PURE__ */ function(d_num) {
	d_num[d_num["x"] = 123] = "x";
	return d_num;
}({});
let e_num$1 = /* @__PURE__ */ function(e_num) {
	e_num[e_num["x"] = 123] = "x";
	return e_num;
}({});
let a_str$1 = /* @__PURE__ */ function(a_str) {
	a_str["x"] = "abc";
	return a_str;
}({});
let b_str$1 = /* @__PURE__ */ function(b_str) {
	b_str["x"] = "abc";
	return b_str;
}({});
let c_str$1 = /* @__PURE__ */ function(c_str) {
	c_str["x"] = "abc";
	return c_str;
}({});
let d_str$1 = /* @__PURE__ */ function(d_str) {
	d_str["x"] = "abc";
	return d_str;
}({});
let e_str$1 = /* @__PURE__ */ function(e_str) {
	e_str["x"] = "abc";
	return e_str;
}({});

//#endregion
//#region entry.ts
inlined = [
	a_num.x,
	b_num$1["x"],
	a_str$1.x,
	b_str$1["x"]
];
not_inlined = [
	c_num$1?.x,
	d_num$1?.["x"],
	e_num$1,
	c_str$1?.x,
	d_str$1?.["x"],
	e_str$1
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
-not_inlined = [c_num?.x, d_num?.["x"], e_num, c_str?.x, d_str?.["x"], e_str];
+var a_num = (function (a_num) {
+    a_num[a_num["x"] = 123] = "x";
+    return a_num;
+})({});
+var b_num$1 = (function (b_num) {
+    b_num[b_num["x"] = 123] = "x";
+    return b_num;
+})({});
+var c_num$1 = (function (c_num) {
+    c_num[c_num["x"] = 123] = "x";
+    return c_num;
+})({});
+var d_num$1 = (function (d_num) {
+    d_num[d_num["x"] = 123] = "x";
+    return d_num;
+})({});
+var e_num$1 = (function (e_num) {
+    e_num[e_num["x"] = 123] = "x";
+    return e_num;
+})({});
+var a_str$1 = (function (a_str) {
+    a_str["x"] = "abc";
+    return a_str;
+})({});
+var b_str$1 = (function (b_str) {
+    b_str["x"] = "abc";
+    return b_str;
+})({});
+var c_str$1 = (function (c_str) {
+    c_str["x"] = "abc";
+    return c_str;
+})({});
+var d_str$1 = (function (d_str) {
+    d_str["x"] = "abc";
+    return d_str;
+})({});
+var e_str$1 = (function (e_str) {
+    e_str["x"] = "abc";
+    return e_str;
+})({});
+inlined = [a_num.x, b_num$1["x"], a_str$1.x, b_str$1["x"]];
+not_inlined = [c_num$1?.x, d_num$1?.["x"], e_num$1, c_str$1?.x, d_str$1?.["x"], e_str$1];

```