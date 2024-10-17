# Reason
1. not support legal comments
# Diff
## /out/entry.js.LEGAL.txt
### esbuild
```js
//! Copyright notice 1
/*
 * @license
 * Copyright notice 2
 */
// @preserve This is another comment

Bundled license information:

some-other-pkg/js/index.js:
  /*
   * @preserve
   * (c) Evil Software Corp
   */

some-pkg/js/index.js:
  //! (c) Good Software Corp
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js.LEGAL.txt
+++ rolldown	
@@ -1,17 +0,0 @@
-//! Copyright notice 1
-/*
- * @license
- * Copyright notice 2
- */
-// @preserve This is another comment
-
-Bundled license information:
-
-some-other-pkg/js/index.js:
-  /*
-   * @preserve
-   * (c) Evil Software Corp
-   */
-
-some-pkg/js/index.js:
-  //! (c) Good Software Corp
\ No newline at end of file

```
## /out/entry.css.LEGAL.txt
### esbuild
```js
/*! Copyright notice 1 */
/*
 * @license
 * Copyright notice 2
 */
/* @preserve This is another comment */

Bundled license information:

some-other-pkg/css/index.css:
  /** @preserve
   * (c) Evil Software Corp
   */

some-pkg/css/index.css:
  /*! (c) Good Software Corp */
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.css.LEGAL.txt
+++ rolldown	
@@ -1,16 +0,0 @@
-/*! Copyright notice 1 */
-/*
- * @license
- * Copyright notice 2
- */
-/* @preserve This is another comment */
-
-Bundled license information:
-
-some-other-pkg/css/index.css:
-  /** @preserve
-   * (c) Evil Software Corp
-   */
-
-some-pkg/css/index.css:
-  /*! (c) Good Software Corp */
\ No newline at end of file

```
## /out/entry.css
### esbuild
```js
a{zoom:2}b{zoom:2}c{zoom:2}.some-other-pkg{zoom:2}
/*! For license information please see entry.css.LEGAL.txt */
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.css
+++ rolldown	
@@ -1,2 +0,0 @@
-a{zoom:2}b{zoom:2}c{zoom:2}.some-other-pkg{zoom:2}
-/*! For license information please see entry.css.LEGAL.txt */
\ No newline at end of file

```