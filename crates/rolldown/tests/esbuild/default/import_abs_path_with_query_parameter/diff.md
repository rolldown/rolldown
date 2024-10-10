# Diff
## /out/entry.js
### esbuild
```js
// Users/user/project/file.txt?foo
var file_default = "This is some text";

// Users/user/project/file.txt#bar
var file_default2 = "This is some text";

// Users/user/project/entry.js
console.log(file_default, file_default2);
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
-var file_default = "This is some text";
-var file_default2 = "This is some text";
-console.log(file_default, file_default2);

```