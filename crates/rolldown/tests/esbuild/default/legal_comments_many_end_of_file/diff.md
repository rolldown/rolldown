# Diff
## /out/entry.js
### esbuild
```js
console.log("in a");console.log("in b");function foo(){console.log("in c");}foo();function bar(){console.log("some-other-pkg")}bar();
//! Copyright notice 1
//! Duplicate comment
/*
 * @license
 * Copyright notice 2
 */
// @preserve This is another comment
/*! Bundled license information:

some-other-pkg/js/index.js:
  (*
   * @preserve
   * (c) Evil Software Corp
   *)
  (*! Duplicate third-party comment *)

some-pkg/js/index.js:
  (*! (c) Good Software Corp *)
  (*! Duplicate third-party comment *)
*/
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