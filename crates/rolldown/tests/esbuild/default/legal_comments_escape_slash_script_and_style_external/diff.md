# Reason
1. not support legal comments
# Diff
## /out/entry.js.LEGAL.txt
### esbuild
```js
/*! </script> */

Bundled license information:

js-pkg/index.js:
  /*! </script> */
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js.LEGAL.txt
+++ rolldown	
@@ -1,6 +0,0 @@
-/*! </script> */
-
-Bundled license information:
-
-js-pkg/index.js:
-  /*! </script> */
\ No newline at end of file

```
## /out/entry.css.LEGAL.txt
### esbuild
```js
/*! </style> */

Bundled license information:

css-pkg/index.css:
  /*! </style> */
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.css.LEGAL.txt
+++ rolldown	
@@ -1,6 +0,0 @@
-/*! </style> */
-
-Bundled license information:
-
-css-pkg/index.css:
-  /*! </style> */
\ No newline at end of file

```
## /out/entry.css
### esbuild
```js
x{y:z}a{b:c}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.css
+++ rolldown	
@@ -1,1 +0,0 @@
-x{y:z}a{b:c}
\ No newline at end of file

```