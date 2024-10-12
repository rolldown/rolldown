# Diff
## /out/entry.js
### esbuild
```js
// entry.ts
function before() {
  console.log(0 /* FOO */);
}
function after() {
  console.log(0 /* FOO */);
}
export {
  after,
  before
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
@@ -1,7 +0,0 @@
-function before() {
-    console.log(0);
-}
-function after() {
-    console.log(0);
-}
-export {after, before};

```