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

```
### diff
```diff
===================================================================
--- esbuild	/out/enum-entry.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log([6, -6, -7, !6, typeof 6], [9, -3, 3 * 6, 3 / 6, 3 % 6, 3 ** 6], [!0, !1, !0, !1, !1, !0, !1, !0], [12, 3, 3], [2, 7, 5], [6, 3, 3, "y", "n"]);

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

```
### diff
```diff
===================================================================
--- esbuild	/out/const-entry.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log([6, -6, -7, !6, typeof 6], [9, -3, 3 * 6, 3 / 6, 3 % 6, 3 ** 6], [!0, !1, !0, !1, !1, !0, !1, !0], [12, 3, 3], [2, 7, 5], [6, 3, 3, "y", "n"]);

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

```
### diff
```diff
===================================================================
--- esbuild	/out/nested-entry.js
+++ rolldown	
@@ -1,4 +0,0 @@
-console.log({
-    "should be 4": 4,
-    "should be 32": 32
-});

```