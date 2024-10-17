# Reason
1. css stabilization
# Diff
## /out/entry.css
### esbuild
```js
/* entry.css */
x {
  y: z;
}
/*! <style>foo<\/style> */
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
-x {
-  y: z;
-}
-/*! <style>foo<\/style> */
\ No newline at end of file

```