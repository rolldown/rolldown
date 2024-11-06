# Reason
1. Our side effects normalization is not right
# Diff
## /out/test.css
### esbuild
```js
/* project/node_modules/pkg/styles.css */
button {
  color: red;
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/test.css
+++ rolldown	
@@ -1,4 +0,0 @@
-/* project/node_modules/pkg/styles.css */
-button {
-  color: red;
-}
\ No newline at end of file

```