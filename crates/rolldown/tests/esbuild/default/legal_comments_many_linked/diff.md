# Diff
## /out/entry.js
### esbuild
```js
console.log("in a");console.log("in b");function foo(){console.log("in c");}foo();function bar(){console.log("some-other-pkg")}bar();
/*! For license information please see entry.js.LEGAL.txt */
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,10 +0,0 @@
-console.log("in a");
-console.log("in b");
-function foo() {
-    console.log("in c");
-}
-foo();
-function bar() {
-    console.log("some-other-pkg");
-}
-bar();

```