# Diff
## /out/entry.js
### esbuild
```js
x;a;
/*! </script> */
/*! Bundled license information:

js-pkg/index.js:
  (*! </script> *)
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
@@ -1,2 +0,0 @@
-x;
-a;

```