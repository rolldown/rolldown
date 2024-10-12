# Diff
## /out.js
### esbuild
```js
// entry.js
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
  a?.[b][c],
  a[b]?.[c]
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
@@ -1,1 +1,1 @@
-console.log([1, 1, 1], [1, 1, 1], [a[b][c], a?.[b][c], a[b]?.[c]]);
+console.log([a.b.c, a?.b.c, a.b?.c], [a["b"]["c"], a?.["b"]["c"], a["b"]?.["c"]], [a[b][c], a?.[b][c], a[b]?.[c]]);

```