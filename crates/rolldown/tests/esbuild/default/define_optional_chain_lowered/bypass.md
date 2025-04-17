# Reason
1. different transformer impl
# Diff
## /out.js
### esbuild
```js
// entry.js
var _a;
console.log([
  1,
  1,
  1
], [
  1,
  1,
  1
], [
  a[b][c],
  a == null ? void 0 : a[b][c],
  (_a = a[b]) == null ? void 0 : _a[c]
]);
```
### rolldown
```js

//#region entry.js
var _a, _a$b;
console.log([
	1,
	1,
	1
], [
	1,
	1,
	1
], [
	a[b][c],
	(_a = a) === null || _a === void 0 ? void 0 : _a[b][c],
	(_a$b = a[b]) === null || _a$b === void 0 ? void 0 : _a$b[c]
]);

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,2 +1,2 @@
-var _a;
-console.log([1, 1, 1], [1, 1, 1], [a[b][c], a == null ? void 0 : a[b][c], (_a = a[b]) == null ? void 0 : _a[c]]);
+var _a, _a$b;
+console.log([1, 1, 1], [1, 1, 1], [a[b][c], (_a = a) === null || _a === void 0 ? void 0 : _a[b][c], (_a$b = a[b]) === null || _a$b === void 0 ? void 0 : _a$b[c]]);

```