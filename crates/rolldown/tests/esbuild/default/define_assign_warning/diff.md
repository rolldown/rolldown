# Diff
## /out/read.js
### esbuild
```js
// read.js
console.log(
  [null, null, null],
  [ident, ident, ident],
  [dot.chain, dot.chain, dot.chain]
);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/read.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log([null, null, null], [ident, ident, ident], [dot.chain, dot.chain, dot.chain]);

```
## /out/write.js
### esbuild
```js
// write.js
console.log(
  [a = 0, b.c = 0, b["c"] = 0],
  [ident = 0, ident = 0, ident = 0],
  [dot.chain = 0, dot.chain = 0, dot.chain = 0]
);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/write.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log([a = 0, b.c = 0, b["c"] = 0], [ident = 0, ident = 0, ident = 0], [dot.chain = 0, dot.chain = 0, dot.chain = 0]);

```