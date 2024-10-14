# Reason
1. not support legal comments
# Diff
## /out/entry.css
### esbuild
```js
x{y:z}a{b:c}
/*! <\/style> */
/*! Bundled license information:

css-pkg/index.css:
  (*! <\/style> *)
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
@@ -1,7 +0,0 @@
-x{y:z}a{b:c}
-/*! <\/style> */
-/*! Bundled license information:
-
-css-pkg/index.css:
-  (*! <\/style> *)
-*/
\ No newline at end of file

```