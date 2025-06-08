# Reason
1. different naming style for shared chunks
# Diff
## /out/a.js
### esbuild
```js
import {
  foo,
  setFoo
} from "./chunk-GX7G2SBE.js";

// a.js
setFoo(123);
console.log(foo);
```
### rolldown
```js
import { b as setFoo, c as foo } from "./shared.js";

//#region a.js
setFoo(123);
console.log(foo);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	a.js
@@ -1,3 +1,3 @@
-import {foo, setFoo} from "./chunk-GX7G2SBE.js";
+import {b as setFoo, c as foo} from "./shared.js";
 setFoo(123);
 console.log(foo);

```
## /out/b.js
### esbuild
```js
import {
  foo
} from "./chunk-GX7G2SBE.js";

// b.js
console.log(foo);
```
### rolldown
```js
import { c as foo } from "./shared.js";

//#region b.js
console.log(foo);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	b.js
@@ -1,2 +1,2 @@
-import {foo} from "./chunk-GX7G2SBE.js";
+import {c as foo} from "./shared.js";
 console.log(foo);

```
## /out/chunk-GX7G2SBE.js
### esbuild
```js
// shared.js
var foo;
function setFoo(value) {
  foo = value;
}

export {
  foo,
  setFoo
};
```
### rolldown
```js
//#region shared.js
let foo;
function setFoo(value) {
	foo = value;
}

//#endregion
export { setFoo as b, foo as c };
```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-GX7G2SBE.js
+++ rolldown	shared.js
@@ -1,5 +1,5 @@
 var foo;
 function setFoo(value) {
     foo = value;
 }
-export {foo, setFoo};
+export {setFoo as b, foo as c};

```