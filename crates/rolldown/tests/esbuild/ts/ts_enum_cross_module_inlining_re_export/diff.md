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
let b = /* @__PURE__ */ function(b) {
	b["x"] = "b";
	return b;
}({});
let c$1 = /* @__PURE__ */ function(c) {
	c["x"] = "c";
	return c;
}({});

//#endregion
//#region entry.js
console.log([
	a.x,
	b.x,
	c$1.x
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
+var b = (function (b) {
+    b["x"] = "b";
+    return b;
+})({});
+var c$1 = (function (c) {
+    c["x"] = "c";
+    return c;
+})({});
+console.log([a.x, b.x, c$1.x]);

```