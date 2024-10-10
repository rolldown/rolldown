## /out/index.js
### esbuild
```js
// src/a.js
var A = 42;

// src/b.js
var B = async () => (await import("./index.js")).A;
export {
  A,
  B
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/index.js
+++ rolldown	
@@ -1,3 +0,0 @@
-var A = 42;
-var B = async () => (await import("./index.js")).A;
-export {A, B};

```
# Diff
## /out/index.js
### esbuild
```js
// src/a.js
var A = 42;

// src/b.js
var B = async () => (await import("./index.js")).A;
export {
  A,
  B
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/index.js
+++ rolldown	
@@ -1,3 +0,0 @@
-var A = 42;
-var B = async () => (await import("./index.js")).A;
-export {A, B};

```