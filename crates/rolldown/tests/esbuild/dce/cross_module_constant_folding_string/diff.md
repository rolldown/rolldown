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

```
### diff
```diff
===================================================================
--- esbuild	/out/enum-entry.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log([typeof "bar"], ["foobar"], [!1, !0, !1, !0, !1, !0, !1, !0], ["bar", "foo", "foo", "y", "n"]);

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

```
### diff
```diff
===================================================================
--- esbuild	/out/const-entry.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var a = "foo", b = "bar";
-console.log([typeof b], [a + b], [a < b, a > b, a <= b, a >= b, a == b, a != b, a === b, a !== b], [a && b, a || b, a ?? b, a ? "y" : "n", b ? "n" : "y"]);

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

```
### diff
```diff
===================================================================
--- esbuild	/out/nested-entry.js
+++ rolldown	
@@ -1,5 +0,0 @@
-var a = "foo", b = "bar", c = "baz";
-console.log({
-    "should be foobarbaz": a + b + c,
-    "should be FOOBARBAZ": "FOOBARBAZ"
-});

```