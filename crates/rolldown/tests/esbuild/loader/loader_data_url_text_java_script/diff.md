# Diff
## /out/entry.js
### esbuild
```js
// <data:text/javascript,console.log('%31%32%33')>
console.log("123");

// <data:text/javascript;base64,Y29uc29sZS5sb2coMjM0KQ==>
console.log(234);

// <data:text/javascript;charset=UTF-8,console.log(%31%32%33)>
console.log(123);

// <data:text/javascript;charset=UTF-8;base64,Y29uc29sZS5sb2coMjM0KQ...>
console.log(234);
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
-console.log("123");
-console.log(234);
-console.log(123);
-console.log(234);

```