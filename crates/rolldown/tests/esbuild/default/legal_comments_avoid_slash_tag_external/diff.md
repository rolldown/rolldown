# Reason
1. not support legal comments
# Diff
## /out/entry.css
### esbuild
```js
/* entry.css */
x {
  y: z;
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.css
+++ rolldown	
@@ -1,4 +0,0 @@
-/* entry.css */
-x {
-  y: z;
-}
\ No newline at end of file

```