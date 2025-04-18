# Reason
1. different inject implementation
# Diff
## /out.js
### esbuild
```js
// inject.js
var foo = 1;
var bar = 2;
var baz = 3;

// entry.js
console.log(
  // These should be fully substituted
  foo,
  bar,
  baz,
  // Should just substitute "import.meta.foo"
  bar.baz,
  // This should not be substituted
  foo.bar
);
```
### rolldown
```js
//#region entry.js
console.log(
	// These should be fully substituted
	import.meta,
	import.meta.foo,
	import.meta.foo.bar,
	// Should just substitute "import.meta.foo"
	import.meta.foo.baz,
	// This should not be substituted
	import.meta.bar
);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,1 @@
-var foo = 1;
-var bar = 2;
-var baz = 3;
-console.log(foo, bar, baz, bar.baz, foo.bar);
+console.log(import.meta, import.meta.foo, import.meta.foo.bar, import.meta.foo.baz, import.meta.bar);

```