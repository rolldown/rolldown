# Diff
## /out.js
### esbuild
```js
// entry.ts
var a = foo.a;
var b = a.b;
var c = b.c;
var bar = c;
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
--- esbuild	/out.js
+++ rolldown	
@@ -1,5 +0,0 @@
-var a = foo.a;
-var b = a.b;
-var c = b.c;
-var bar = c;
-export {bar};

```