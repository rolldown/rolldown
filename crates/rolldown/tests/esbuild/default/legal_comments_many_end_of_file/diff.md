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
## /out/entry.css
### esbuild
```js
a{zoom:2}b{zoom:2}c{zoom:2}.some-other-pkg{zoom:2}
/*! Copyright notice 1 */
/*! Duplicate comment */
/*
 * @license
 * Copyright notice 2
 */
/* @preserve This is another comment */
/*! Bundled license information:

some-other-pkg/css/index.css:
  (*! Duplicate third-party comment *)
  (** @preserve
   * (c) Evil Software Corp
   *)

some-pkg/css/index.css:
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
--- esbuild	/out/entry.css
+++ rolldown	
@@ -1,20 +0,0 @@
-a{zoom:2}b{zoom:2}c{zoom:2}.some-other-pkg{zoom:2}
-/*! Copyright notice 1 */
-/*! Duplicate comment */
-/*
- * @license
- * Copyright notice 2
- */
-/* @preserve This is another comment */
-/*! Bundled license information:
-
-some-other-pkg/css/index.css:
-  (*! Duplicate third-party comment *)
-  (** @preserve
-   * (c) Evil Software Corp
-   *)
-
-some-pkg/css/index.css:
-  (*! (c) Good Software Corp *)
-  (*! Duplicate third-party comment *)
-*/
\ No newline at end of file

```