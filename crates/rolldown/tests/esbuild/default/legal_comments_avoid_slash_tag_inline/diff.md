# Diff
## /out/entry.js
### esbuild
```js
// entry.js
//! <script>foo<\/script>
var x;
export {
  x
};
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