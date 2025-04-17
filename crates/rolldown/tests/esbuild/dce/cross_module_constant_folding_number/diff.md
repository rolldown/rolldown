# Reason
1.not support cross module constant folding
# Diff
## /out/enum-entry.js
### esbuild
```js
// enum-entry.ts
console.log([
  6,
  -6,
  -7,
  !6 /* b */,
  typeof 6 /* b */
], [
  9,
  -3,
  3 /* a */ * 6 /* b */,
  3 /* a */ / 6 /* b */,
  3 /* a */ % 6 /* b */,
  3 /* a */ ** 6 /* b */
], [
  !0,
  !1,
  !0,
  !1,
  !1,
  !0,
  !1,
  !0
], [
  12,
  3,
  3
], [
  2,
  7,
  5
], [
  6 /* b */,
  3 /* a */,
  3 /* a */,
  "y",
  "n"
]);
```
### rolldown
```js

//#region enum-constants.ts
let x = /* @__PURE__ */ function(x$1) {
	x$1[x$1["a"] = 3] = "a";
	x$1[x$1["b"] = 6] = "b";
	return x$1;
}({});

//#region enum-entry.ts
console.log([
	+x.b,
	-x.b,
	~x.b,
	!x.b,
	typeof x.b
], [
	x.a + x.b,
	x.a - x.b,
	x.a * x.b,
	x.a / x.b,
	x.a % x.b,
	x.a ** x.b
], [
	x.a < x.b,
	x.a > x.b,
	x.a <= x.b,
	x.a >= x.b,
	x.a == x.b,
	x.a != x.b,
	x.a === x.b,
	x.a !== x.b
], [
	x.b << 1,
	x.b >> 1,
	x.b >>> 1
], [
	x.a & x.b,
	x.a | x.b,
	x.a ^ x.b
], [
	x.a && x.b,
	x.a || x.b,
	x.a ?? x.b,
	x.a ? "y" : "n",
	!x.b ? "y" : "n"
]);

```
### diff
```diff
===================================================================
--- esbuild	/out/enum-entry.js
+++ rolldown	enum-entry.js
@@ -1,1 +1,6 @@
-console.log([6, -6, -7, !6, typeof 6], [9, -3, 3 * 6, 3 / 6, 3 % 6, 3 ** 6], [!0, !1, !0, !1, !1, !0, !1, !0], [12, 3, 3], [2, 7, 5], [6, 3, 3, "y", "n"]);
+var x = (function (x$1) {
+    x$1[x$1["a"] = 3] = "a";
+    x$1[x$1["b"] = 6] = "b";
+    return x$1;
+})({});
+console.log([+x.b, -x.b, ~x.b, !x.b, typeof x.b], [x.a + x.b, x.a - x.b, x.a * x.b, x.a / x.b, x.a % x.b, x.a ** x.b], [x.a < x.b, x.a > x.b, x.a <= x.b, x.a >= x.b, x.a == x.b, x.a != x.b, x.a === x.b, x.a !== x.b], [x.b << 1, x.b >> 1, x.b >>> 1], [x.a & x.b, x.a | x.b, x.a ^ x.b], [x.a && x.b, x.a || x.b, x.a ?? x.b, x.a ? "y" : "n", !x.b ? "y" : "n"]);

```
## /out/const-entry.js
### esbuild
```js
// const-entry.js
console.log([
  6,
  -6,
  -7,
  !6,
  typeof 6
], [
  9,
  -3,
  3 * 6,
  3 / 6,
  3 % 6,
  3 ** 6
], [
  !0,
  !1,
  !0,
  !1,
  !1,
  !0,
  !1,
  !0
], [
  12,
  3,
  3
], [
  2,
  7,
  5
], [
  6,
  3,
  3,
  "y",
  "n"
]);
```
### rolldown
```js

//#region const-constants.js
const a = 3;
const b = 6;

//#region const-entry.js
console.log([
	+b,
	-b,
	~b,
	!b,
	typeof b
], [
	a + b,
	a - b,
	a * b,
	a / b,
	a % b,
	a ** b
], [
	a < b,
	a > b,
	a <= b,
	a >= b,
	a == b,
	a != b,
	a === b,
	a !== b
], [
	b << 1,
	b >> 1,
	b >>> 1
], [
	a & b,
	a | b,
	a ^ b
], [
	a && b,
	a || b,
	a ?? b,
	a ? "y" : "n",
	!b ? "y" : "n"
]);

```
### diff
```diff
===================================================================
--- esbuild	/out/const-entry.js
+++ rolldown	const-entry.js
@@ -1,1 +1,3 @@
-console.log([6, -6, -7, !6, typeof 6], [9, -3, 3 * 6, 3 / 6, 3 % 6, 3 ** 6], [!0, !1, !0, !1, !1, !0, !1, !0], [12, 3, 3], [2, 7, 5], [6, 3, 3, "y", "n"]);
+var a = 3;
+var b = 6;
+console.log([+b, -b, ~b, !b, typeof b], [a + b, a - b, a * b, a / b, a % b, a ** b], [a < b, a > b, a <= b, a >= b, a == b, a != b, a === b, a !== b], [b << 1, b >> 1, b >>> 1], [a & b, a | b, a ^ b], [a && b, a || b, a ?? b, a ? "y" : "n", !b ? "y" : "n"]);

```
## /out/nested-entry.js
### esbuild
```js
// nested-entry.ts
console.log({
  "should be 4": 4,
  "should be 32": 32
});
```
### rolldown
```js

//#region nested-constants.ts
const a = 2;
const b = 4;
const c = 8;
let x = /* @__PURE__ */ function(x$1) {
	x$1[x$1["a"] = 16] = "a";
	x$1[x$1["b"] = 32] = "b";
	x$1[x$1["c"] = 64] = "c";
	return x$1;
}({});

//#region nested-entry.ts
console.log({
	"should be 4": ~(~a & ~b) & (b | c),
	"should be 32": ~(~x.a & ~x.b) & (x.b | x.c)
});

```
### diff
```diff
===================================================================
--- esbuild	/out/nested-entry.js
+++ rolldown	nested-entry.js
@@ -1,4 +1,13 @@
+var a = 2;
+var b = 4;
+var c = 8;
+var x = (function (x$1) {
+    x$1[x$1["a"] = 16] = "a";
+    x$1[x$1["b"] = 32] = "b";
+    x$1[x$1["c"] = 64] = "c";
+    return x$1;
+})({});
 console.log({
-    "should be 4": 4,
-    "should be 32": 32
+    "should be 4": ~(~a & ~b) & (b | c),
+    "should be 32": ~(~x.a & ~x.b) & (x.b | x.c)
 });

```