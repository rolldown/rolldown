## /out/entry.js
### esbuild
```js
// entry.js
import("./foo-R2VCCZUR.js").then(({ bar }) => console.log(bar));
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,1 +0,0 @@
-import("./foo-R2VCCZUR.js").then(({bar}) => console.log(bar));

```
## /out/foo-R2VCCZUR.js
### esbuild
```js
// foo.js
var bar = 123;
export {
  bar
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/foo-R2VCCZUR.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var bar = 123;
-export {bar};

```
# Diff
## /out/entry.js
### esbuild
```js
// entry.js
import("./foo-R2VCCZUR.js").then(({ bar }) => console.log(bar));
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,1 +0,0 @@
-import("./foo-R2VCCZUR.js").then(({bar}) => console.log(bar));

```
## /out/foo-R2VCCZUR.js
### esbuild
```js
// foo.js
var bar = 123;
export {
  bar
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/foo-R2VCCZUR.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var bar = 123;
-export {bar};

```