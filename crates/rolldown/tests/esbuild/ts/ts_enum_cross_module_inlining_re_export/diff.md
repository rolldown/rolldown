# Reason
1. not support const enum inline
# Diff
## /out/entry.js
### esbuild
```js
// entry.js
console.log([
  "a" /* x */,
  "b" /* x */,
  "c" /* x */
]);
```
### rolldown
```js

//#region enums.ts
let a = /* @__PURE__ */ function(a) {
	a["x"] = "a";
	return a;
}({});
let b$1 = /* @__PURE__ */ function(b) {
	b["x"] = "b";
	return b;
}({});
let c = /* @__PURE__ */ function(c) {
	c["x"] = "c";
	return c;
}({});

//#endregion
//#region entry.js
console.log([
	a.x,
	b$1.x,
	c.x
]);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,1 +1,13 @@
-console.log(["a", "b", "c"]);
+var a = (function (a) {
+    a["x"] = "a";
+    return a;
+})({});
+var b$1 = (function (b) {
+    b["x"] = "b";
+    return b;
+})({});
+var c = (function (c) {
+    c["x"] = "c";
+    return c;
+})({});
+console.log([a.x, b$1.x, c.x]);

```