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

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,3 +0,0 @@
-var file_foo_default = "foo";
-var file_bar_default = "bar";
-console.log(file_foo_default, file_bar_default);

```