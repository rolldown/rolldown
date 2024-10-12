# Diff
## /out/entry.js
### esbuild
```js
// a.js
console.log("in a");

// b.js
console.log("in b");

// c.js
console.log("in c");
/*! For license information please see entry.js.LEGAL.txt */
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,3 +0,0 @@
-console.log("in a");
-console.log("in b");
-console.log("in c");

```
## /out/entry.css
### esbuild
```js
/* a.css */
a {
  zoom: 2;
}

/* b.css */
b {
  zoom: 2;
}

/* c.css */
c {
  zoom: 2;
}

/* entry.css */
/*! For license information please see entry.css.LEGAL.txt */
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.css
+++ rolldown	
@@ -1,17 +0,0 @@
-/* a.css */
-a {
-  zoom: 2;
-}
-
-/* b.css */
-b {
-  zoom: 2;
-}
-
-/* c.css */
-c {
-  zoom: 2;
-}
-
-/* entry.css */
-/*! For license information please see entry.css.LEGAL.txt */
\ No newline at end of file

```