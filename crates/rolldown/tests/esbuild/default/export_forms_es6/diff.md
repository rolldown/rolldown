# Diff
## /out.js
### esbuild
```js
// a.js
var abc = void 0;

// b.js
var b_exports = {};
__export(b_exports, {
  xyz: () => xyz
});
var xyz = null;

// entry.js
var entry_default = 123;
var v = 234;
var l = 234;
var c = 234;
function Fn() {
}
var Class = class {
};
export {
  Class as C,
  Class,
  Fn,
  abc,
  b_exports as b,
  c,
  entry_default as default,
  l,
  v
};
```
### rolldown
```js

//#region rolldown:runtime
var __defProp = Object.defineProperty;
var __export = (target, all) => {
	for (var name in all) __defProp(target, name, {
		get: all[name],
		enumerable: true
	});
};

//#region a.js
const abc = void 0;

//#region b.js
var b_exports = {};
__export(b_exports, { xyz: () => xyz });
const xyz = null;

//#region entry.js
var entry_default = 123;
var v = 234;
let l = 234;
const c = 234;
function Fn() {}
var Class = class {};

export { Class as C, Class, Fn, abc, b_exports as b, c, entry_default as default, l, v };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,11 @@
+var __defProp = Object.defineProperty;
+var __export = (target, all) => {
+    for (var name in all) __defProp(target, name, {
+        get: all[name],
+        enumerable: true
+    });
+};
 var abc = void 0;
 var b_exports = {};
 __export(b_exports, {
     xyz: () => xyz

```