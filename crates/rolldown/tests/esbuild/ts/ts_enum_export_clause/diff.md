# Reason
1. not support const enum inline
# Diff
## /out/entry.js
### esbuild
```js
// entry.ts
console.log([
  1 /* A */,
  2 /* B */,
  3 /* C */,
  4 /* D */
]);
```
### rolldown
```js

//#region enums.ts
let A = /* @__PURE__ */ function(A$1) {
	A$1[A$1["A"] = 1] = "A";
	return A$1;
}({});
var B = /* @__PURE__ */ function(B$1) {
	B$1[B$1["B"] = 2] = "B";
	return B$1;
}(B || {});
let C = /* @__PURE__ */ function(C$1) {
	C$1[C$1["C"] = 3] = "C";
	return C$1;
}({});
var D = /* @__PURE__ */ function(D$1) {
	D$1[D$1["D"] = 4] = "D";
	return D$1;
}(D || {});
//#endregion

//#region entry.ts
console.log([
	A.A,
	B.B,
	C.C,
	D.D
]);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,1 +1,17 @@
-console.log([1, 2, 3, 4]);
+var A = (function (A$1) {
+    A$1[A$1["A"] = 1] = "A";
+    return A$1;
+})({});
+var B = (function (B$1) {
+    B$1[B$1["B"] = 2] = "B";
+    return B$1;
+})(B || ({}));
+var C = (function (C$1) {
+    C$1[C$1["C"] = 3] = "C";
+    return C$1;
+})({});
+var D = (function (D$1) {
+    D$1[D$1["D"] = 4] = "D";
+    return D$1;
+})(D || ({}));
+console.log([A.A, B.B, C.C, D.D]);

```