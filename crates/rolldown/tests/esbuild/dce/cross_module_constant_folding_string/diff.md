# Reason
1.not support cross module constant folding
# Diff
## /out/enum-entry.js
### esbuild
```js
// enum-entry.ts
console.log([
  typeof "bar" /* b */
], [
  "foobar"
], [
  !1,
  !0,
  !1,
  !0,
  !1,
  !0,
  !1,
  !0
], [
  "bar" /* b */,
  "foo" /* a */,
  "foo" /* a */,
  "y",
  "n"
]);
```
### rolldown
```js

//#region enum-constants.ts
let x = /* @__PURE__ */ function(x$1) {
	x$1["a"] = "foo";
	x$1["b"] = "bar";
	return x$1;
}({});

//#region enum-entry.ts
console.log([typeof x.b], [x.a + x.b], [
	x.a < x.b,
	x.a > x.b,
	x.a <= x.b,
	x.a >= x.b,
	x.a == x.b,
	x.a != x.b,
	x.a === x.b,
	x.a !== x.b
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
-console.log([typeof "bar"], ["foobar"], [!1, !0, !1, !0, !1, !0, !1, !0], ["bar", "foo", "foo", "y", "n"]);
+var x = (function (x$1) {
+    x$1["a"] = "foo";
+    x$1["b"] = "bar";
+    return x$1;
+})({});
+console.log([typeof x.b], [x.a + x.b], [x.a < x.b, x.a > x.b, x.a <= x.b, x.a >= x.b, x.a == x.b, x.a != x.b, x.a === x.b, x.a !== x.b], [x.a && x.b, x.a || x.b, x.a ?? x.b, x.a ? "y" : "n", !x.b ? "y" : "n"]);

```
## /out/const-entry.js
### esbuild
```js
// const-constants.js
var a = "foo", b = "bar";

// const-entry.js
console.log([
  typeof b
], [
  a + b
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
  a && b,
  a || b,
  a ?? b,
  a ? "y" : "n",
  b ? "n" : "y"
]);
```
### rolldown
```js

//#region const-constants.js
const a = "foo";
const b = "bar";

//#region const-entry.js
console.log([typeof b], [a + b], [
	a < b,
	a > b,
	a <= b,
	a >= b,
	a == b,
	a != b,
	a === b,
	a !== b
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
@@ -1,2 +1,3 @@
-var a = "foo", b = "bar";
-console.log([typeof b], [a + b], [a < b, a > b, a <= b, a >= b, a == b, a != b, a === b, a !== b], [a && b, a || b, a ?? b, a ? "y" : "n", b ? "n" : "y"]);
+var a = "foo";
+var b = "bar";
+console.log([typeof b], [a + b], [a < b, a > b, a <= b, a >= b, a == b, a != b, a === b, a !== b], [a && b, a || b, a ?? b, a ? "y" : "n", !b ? "y" : "n"]);

```
## /out/nested-entry.js
### esbuild
```js
// nested-constants.ts
var a = "foo", b = "bar", c = "baz";

// nested-entry.ts
console.log({
  "should be foobarbaz": a + b + c,
  "should be FOOBARBAZ": "FOOBARBAZ"
});
```
### rolldown
```js

//#region nested-constants.ts
const a = "foo";
const b = "bar";
const c = "baz";
let x = /* @__PURE__ */ function(x$1) {
	x$1["a"] = "FOO";
	x$1["b"] = "BAR";
	x$1["c"] = "BAZ";
	return x$1;
}({});

//#region nested-entry.ts
console.log({
	"should be foobarbaz": a + b + c,
	"should be FOOBARBAZ": x.a + x.b + x.c
});

```
### diff
```diff
===================================================================
--- esbuild	/out/nested-entry.js
+++ rolldown	nested-entry.js
@@ -1,5 +1,13 @@
-var a = "foo", b = "bar", c = "baz";
+var a = "foo";
+var b = "bar";
+var c = "baz";
+var x = (function (x$1) {
+    x$1["a"] = "FOO";
+    x$1["b"] = "BAR";
+    x$1["c"] = "BAZ";
+    return x$1;
+})({});
 console.log({
     "should be foobarbaz": a + b + c,
-    "should be FOOBARBAZ": "FOOBARBAZ"
+    "should be FOOBARBAZ": x.a + x.b + x.c
 });

```