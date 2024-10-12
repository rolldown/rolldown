# Diff
## /out/entry.css
### esbuild
```js
/* <data:text/css,body{color:%72%65%64}> */
body {
  color: red;
}

/* <data:text/css;base64,Ym9keXtiYWNrZ3JvdW5kOmJsdWV9> */
body {
  background: blue;
}

/* <data:text/css;charset=UTF-8,body{color:%72%65%64}> */
body {
  color: red;
}

/* <data:text/css;charset=UTF-8;base64,Ym9keXtiYWNrZ3JvdW5kOmJsdWV9> */
body {
  background: blue;
}

/* entry.css */
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.css
+++ rolldown	
@@ -1,21 +0,0 @@
-/* <data:text/css,body{color:%72%65%64}> */
-body {
-  color: red;
-}
-
-/* <data:text/css;base64,Ym9keXtiYWNrZ3JvdW5kOmJsdWV9> */
-body {
-  background: blue;
-}
-
-/* <data:text/css;charset=UTF-8,body{color:%72%65%64}> */
-body {
-  color: red;
-}
-
-/* <data:text/css;charset=UTF-8;base64,Ym9keXtiYWNrZ3JvdW5kOmJsdWV9> */
-body {
-  background: blue;
-}
-
-/* entry.css */
\ No newline at end of file

```