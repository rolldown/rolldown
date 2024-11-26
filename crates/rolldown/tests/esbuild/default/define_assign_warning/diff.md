# Reason
1. oxc define not support computed member expr
2. not support member expr with write
# Diff
## /out/write.js
### esbuild
```js
// write.js
console.log(
  [a = 0, b.c = 0, b["c"] = 0],
  [ident = 0, ident = 0, ident = 0],
  [dot.chain = 0, dot.chain = 0, dot.chain = 0]
);
```
### rolldown
```js

//#region write.js
console.log([
	a = 0,
	b.c = 0,
	b["c"] = 0
], [
	d = 0,
	e.f = 0,
	e["f"] = 0
], [
	g = 0,
	h.i = 0,
	h["i"] = 0
]);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/write.js
+++ rolldown	write.js
@@ -1,1 +1,1 @@
-console.log([a = 0, b.c = 0, b["c"] = 0], [ident = 0, ident = 0, ident = 0], [dot.chain = 0, dot.chain = 0, dot.chain = 0]);
+console.log([a = 0, b.c = 0, b["c"] = 0], [d = 0, e.f = 0, e["f"] = 0], [g = 0, h.i = 0, h["i"] = 0]);

```