# Diff
## /out/entry.js
### esbuild
```js
// file#foo.txt
var file_foo_default = "foo";

// file#bar.txt
var file_bar_default = "bar";

// entry.js
console.log(file_foo_default, file_bar_default);
```
### rolldown
```js

//#region file#foo.txt
"foo";

//#endregion
//#region file#bar.txt
"bar";

//#endregion
//#region entry.js
console.log(file_foo_default, file_bar_default);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,3 +1,3 @@
-var file_foo_default = "foo";
-var file_bar_default = "bar";
+"foo";
+"bar";
 console.log(file_foo_default, file_bar_default);

```