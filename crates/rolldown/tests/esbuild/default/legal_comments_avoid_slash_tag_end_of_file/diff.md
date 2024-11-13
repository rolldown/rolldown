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
/*! <style>foo<\/style> */
```
### rolldown
```js
/*! <style>foo</style> */
x { y: z }

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.css
+++ rolldown	entry2.css
@@ -1,5 +1,2 @@
-/* entry.css */
-x {
-  y: z;
-}
-/*! <style>foo<\/style> */
\ No newline at end of file
+/*! <style>foo</style> */
+x { y: z }

```