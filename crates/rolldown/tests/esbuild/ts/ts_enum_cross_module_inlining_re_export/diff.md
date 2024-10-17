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
let a = function(a$1) {
	a$1["x"] = "a";
	return a$1;
}({});
let b = function(b$1) {
	b$1["x"] = "b";
	return b$1;
}({});
let c = function(c$1) {
	c$1["x"] = "c";
	return c$1;
}({});

//#endregion
//#region entry.js
console.log([
	a.x,
	b.x,
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
+var a = (function (a$1) {
+    a$1["x"] = "a";
+    return a$1;
+})({});
+var b = (function (b$1) {
+    b$1["x"] = "b";
+    return b$1;
+})({});
+var c = (function (c$1) {
+    c$1["x"] = "c";
+    return c$1;
+})({});
+console.log([a.x, b.x, c.x]);

```