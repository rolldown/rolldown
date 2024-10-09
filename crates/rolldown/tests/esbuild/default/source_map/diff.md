# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/src/bar.js
function bar() {
  throw new Error("test");
}

// Users/user/project/src/data.txt
var data_default = "#2041";

// Users/user/project/src/entry.js
function foo() {
  bar();
}
foo();
console.log(data_default);
//# sourceMappingURL=out.js.map
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	
@@ -1,9 +0,0 @@
-function bar() {
-    throw new Error("test");
-}
-var data_default = "#2041";
-function foo() {
-    bar();
-}
-foo();
-console.log(data_default);

```