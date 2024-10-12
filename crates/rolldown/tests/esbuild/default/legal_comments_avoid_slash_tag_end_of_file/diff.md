# Diff
## /out/entry.js
### esbuild
```js
// entry.js
var x;
export {
  x
};
//! <script>foo<\/script>
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var x;
-export {x};

```
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