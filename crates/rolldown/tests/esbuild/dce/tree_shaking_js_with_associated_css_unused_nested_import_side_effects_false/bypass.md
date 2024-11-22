# Reason
1. Since the `sideEffects: false`, and the `ImportDeclaration` is just plain, the whole sub tree (including css file) should be eliminated
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