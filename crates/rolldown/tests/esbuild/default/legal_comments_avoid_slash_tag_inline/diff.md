# Reason
1. not support legal comments
# Diff
## /out/entry.css
### esbuild
```js
/* entry.css */
/*! <style>foo<\/style> */
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
@@ -1,5 +0,0 @@
-/* entry.css */
-/*! <style>foo<\/style> */
-x {
-  y: z;
-}
\ No newline at end of file

```