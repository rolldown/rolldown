# Reason
1. esbuild generate debug id for each css file
2. sub optimal
# Diff
## /out/test.css
### esbuild
```js
/* project/node_modules/pkg/button.css */
button {
  color: red;
}

/* project/node_modules/pkg/menu.css */
menu {
  color: red;
}
```
### rolldown
```js
button { color: red }
menu { color: red }

```
### diff
```diff
===================================================================
--- esbuild	/out/test.css
+++ rolldown	test.css
@@ -1,9 +1,2 @@
-/* project/node_modules/pkg/button.css */
-button {
-  color: red;
-}
-
-/* project/node_modules/pkg/menu.css */
-menu {
-  color: red;
-}
\ No newline at end of file
+button { color: red }
+menu { color: red }

```