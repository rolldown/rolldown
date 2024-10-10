# Diff
## /out/entry.js
### esbuild
```js
// a.js
var a = 1;

// b.js
var b = 3;

// c.js
var c = 4;
export {
  a,
  b,
  c
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
@@ -1,4 +0,0 @@
-var a = 1;
-var b = 3;
-var c = 4;
-export {a, b, c};

```