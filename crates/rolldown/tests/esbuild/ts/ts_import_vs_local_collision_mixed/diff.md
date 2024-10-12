# Diff
## /out.js
### esbuild
```js
// other.ts
var real = 123;

// entry.ts
var a;
var b = 0;
var c;
function d() {
}
var e = class {
};
console.log(a, b, c, d, e, real);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,7 +0,0 @@
-var real = 123;
-var a;
-var b = 0;
-var c;
-function d() {}
-var e = class {};
-console.log(a, b, c, d, e, real);

```