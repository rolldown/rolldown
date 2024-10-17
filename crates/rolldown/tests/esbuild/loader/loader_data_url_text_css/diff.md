# Reason
1. css stabilization
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
body{color:red}
body{background:blue}
body{color:red}
body{background:blue}


```
### diff
```diff
===================================================================
--- esbuild	/out/entry.css
+++ rolldown	entry.css
@@ -1,21 +1,5 @@
-/* <data:text/css,body{color:%72%65%64}> */
-body {
-  color: red;
-}
+body{color:red}
+body{background:blue}
+body{color:red}
+body{background:blue}
 
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