# Reason
1. not support legal comments
# Diff
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