# Diff
## /out.js
### esbuild
```js
// foo.ts
console.log({
  "should have comments": [
    1 /* %/* */,
    1 /* %/* */
  ],
  "should not have comments": [
    2,
    2
  ]
});
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,4 +0,0 @@
-console.log({
-    "should have comments": [1, 1],
-    "should not have comments": [2, 2]
-});

```