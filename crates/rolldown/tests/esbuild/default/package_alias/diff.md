# Diff
## /out.js
### esbuild
```js
// node_modules/alias1/index.js
console.log(1);

// node_modules/alias2/foo.js
console.log(2);

// node_modules/alias3/index.js
console.log(3);

// node_modules/alias4/index.js
console.log(4);

// node_modules/alias5/foo.js
console.log(5);

// alias6/dir/index.js
console.log(6);

// alias7/dir/foo/index.js
console.log(7);

// alias8/dir/pkg8/index.js
console.log(8);

// alias9/some/file.js
console.log(9);

// node_modules/prefix-foo/index.js
console.log(10);

// node_modules/@scope/prefix-foo/index.js
console.log(11);
```
### rolldown
```js
import "pkg1";
import "pkg2/foo";
import "pkg3";
import "@scope/pkg4";
import "@scope/pkg5/foo";
import "@abs-path/pkg6";
import "@abs-path/pkg7/foo";
import "@scope-only/pkg8";
import "slash/";

//#region node_modules/prefix-foo/index.js
console.log(10);

//#endregion
//#region node_modules/@scope/prefix-foo/index.js
console.log(11);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,11 +1,11 @@
-console.log(1);
-console.log(2);
-console.log(3);
-console.log(4);
-console.log(5);
-console.log(6);
-console.log(7);
-console.log(8);
-console.log(9);
+import "pkg1";
+import "pkg2/foo";
+import "pkg3";
+import "@scope/pkg4";
+import "@scope/pkg5/foo";
+import "@abs-path/pkg6";
+import "@abs-path/pkg7/foo";
+import "@scope-only/pkg8";
+import "slash/";
 console.log(10);
 console.log(11);

```