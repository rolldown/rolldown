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

//#region entry.ts
let a;
const b = 0;
var c;
function d() {}
class e {}
console.log(a, b, c, d, e);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,6 @@
 var a;
 var b = 0;
 var c;
 function d() {}
-var e = class {};
+class e {}
 console.log(a, b, c, d, e);

```