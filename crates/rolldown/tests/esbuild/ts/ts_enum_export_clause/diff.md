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
let A = /* @__PURE__ */ function(A) {
	A[A["A"] = 1] = "A";
	return A;
}({});
var B = /* @__PURE__ */ function(B) {
	B[B["B"] = 2] = "B";
	return B;
}(B || {});
let c = /* @__PURE__ */ function(C) {
	C[C["C"] = 3] = "C";
	return C;
}({});
var dd = /* @__PURE__ */ function(D) {
	D[D["D"] = 4] = "D";
	return D;
}(dd || {});

//#endregion
//#region entry.ts
console.log([
	A.A,
	B.B,
	c.C,
	dd.D
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
+var A = (function (A) {
+    A[A["A"] = 1] = "A";
+    return A;
+})({});
+var B = (function (B) {
+    B[B["B"] = 2] = "B";
+    return B;
+})(B || ({}));
+var c = (function (C) {
+    C[C["C"] = 3] = "C";
+    return C;
+})({});
+var dd = (function (D) {
+    D[D["D"] = 4] = "D";
+    return D;
+})(dd || ({}));
+console.log([A.A, B.B, c.C, dd.D]);

```