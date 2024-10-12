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

//#region other.ts
let real = 123;

//#endregion
//#region entry.ts
let a;
const b = 0;
var c;
function d() {}
class e {}
console.log(a, b, c, d, e, real);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -2,6 +2,6 @@
 var a;
 var b = 0;
 var c;
 function d() {}
-var e = class {};
+class e {}
 console.log(a, b, c, d, e, real);

```