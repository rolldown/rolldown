# Diff
## /out.js
### esbuild
```js
// entry.ts
var a;
var b = 0;
var c;
function d() {
}
var e = class {
};
console.log(a, b, c, d, e);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,6 +0,0 @@
-var a;
-var b = 0;
-var c;
-function d() {}
-var e = class {};
-console.log(a, b, c, d, e);

```