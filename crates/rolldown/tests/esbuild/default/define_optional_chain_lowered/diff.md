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
console.log([
	a.b.c,
	a?.b.c,
	a.b?.c
], [
	a["b"]["c"],
	a?.["b"]["c"],
	a["b"]?.["c"]
], [
	a[b][c],
	a?.[b][c],
	a[b]?.[c]
]);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,2 +1,1 @@
-var _a;
-console.log([1, 1, 1], [1, 1, 1], [a[b][c], a == null ? void 0 : a[b][c], (_a = a[b]) == null ? void 0 : _a[c]]);
+console.log([a.b.c, a?.b.c, a.b?.c], [a["b"]["c"], a?.["b"]["c"], a["b"]?.["c"]], [a[b][c], a?.[b][c], a[b]?.[c]]);

```