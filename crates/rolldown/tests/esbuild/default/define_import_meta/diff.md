# Reason
1. oxc define
# Diff
## /out.js
### esbuild
```js
// entry.js
console.log(
  // These should be fully substituted
  1,
  2,
  3,
  // Should just substitute "import.meta.foo"
  2 .baz,
  // This should not be substituted
  1 .bar
);
```
### rolldown
```js

//#region entry.js
console.log(
	1,
	2,
	3,
	// Should just substitute "import.meta.foo"
	2 .baz,
	3
);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,1 +1,1 @@
-console.log(1, 2, 3, (2).baz, (1).bar);
+console.log(1, 2, 3, (2).baz, 3);

```