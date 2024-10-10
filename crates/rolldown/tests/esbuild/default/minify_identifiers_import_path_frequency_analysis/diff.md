# Diff
## /out/import.js
### esbuild
```js
var o=123;console.log(o,"no identifier in this file should be named W, X, Y, or Z");
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/import.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var o = 123;
-console.log(o, "no identifier in this file should be named W, X, Y, or Z");

```
## /out/require.js
### esbuild
```js
var i=r((t,e)=>{e.exports=123});var s=i();console.log(s,"no identifier in this file should be named A, B, C, or D");
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/require.js
+++ rolldown	
@@ -1,5 +0,0 @@
-var i = r((t, e) => {
-    e.exports = 123;
-});
-var s = i();
-console.log(s, "no identifier in this file should be named A, B, C, or D");

```