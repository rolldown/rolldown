## /out/a.js
### esbuild
```js
// a.js
import("./b.js");
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	
@@ -1,1 +0,0 @@
-import("./b.js");

```
## /out/b.js
### esbuild
```js
// b.js
var b_default = 1;
export {
  b_default as default
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var b_default = 1;
-export {b_default as default};

```
# Diff
## /out/a.js
### esbuild
```js
// a.js
import("./b.js");
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	
@@ -1,1 +0,0 @@
-import("./b.js");

```
## /out/b.js
### esbuild
```js
// b.js
var b_default = 1;
export {
  b_default as default
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var b_default = 1;
-export {b_default as default};

```