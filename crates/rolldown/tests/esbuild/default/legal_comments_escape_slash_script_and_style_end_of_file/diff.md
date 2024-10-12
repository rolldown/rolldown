# Diff
## /out/entry.js
### esbuild
```js
x;a;
/*! <\/script> */
/*! Bundled license information:

js-pkg/index.js:
  (*! <\/script> *)
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