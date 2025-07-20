# Reason
1. rolldown is not ts aware after ts transformation, We can't aware that `Test` is just a type
# Diff
## /out.js
### esbuild
```js
// a.ts
var a_exports = {};
__export(a_exports, {
  foo: () => foo
});
var foo = 123;

// b.ts
var b_exports = {};
__export(b_exports, {
  foo: () => foo2
});
var foo2 = 123;

// c.ts
var c_exports = {};
__export(c_exports, {
  foo: () => foo3
});
var foo3 = 123;

// d.ts
var d_exports = {};
__export(d_exports, {
  foo: () => foo4
});
var foo4 = 123;

// entry.ts
console.log(a_exports, b_exports, c_exports, d_exports);
```
### rolldown
```js
// HIDDEN [rolldown:runtime]
//#region a.ts
var a_exports = {};
__export(a_exports, { foo: () => foo$3 });
let foo$3 = 123;

//#endregion
//#region b.ts
var b_exports = {};
__export(b_exports, { foo: () => foo$2 });
let foo$2 = 123;

//#endregion
//#region test.ts
var Test = void 0;

//#endregion
//#region c.ts
var c_exports = {};
__export(c_exports, {
	Test: () => Test,
	foo: () => foo$1
});
let foo$1 = 123;

//#endregion
//#region d.ts
var d_exports = {};
__export(d_exports, {
	Test: () => Test,
	foo: () => foo
});
let foo = 123;

//#endregion
//#region entry.ts
console.log(a_exports, b_exports, c_exports, d_exports);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,21 +1,24 @@
 var a_exports = {};
 __export(a_exports, {
-    foo: () => foo
+    foo: () => foo$3
 });
-var foo = 123;
+var foo$3 = 123;
 var b_exports = {};
 __export(b_exports, {
-    foo: () => foo2
+    foo: () => foo$2
 });
-var foo2 = 123;
+var foo$2 = 123;
+var Test = void 0;
 var c_exports = {};
 __export(c_exports, {
-    foo: () => foo3
+    Test: () => Test,
+    foo: () => foo$1
 });
-var foo3 = 123;
+var foo$1 = 123;
 var d_exports = {};
 __export(d_exports, {
-    foo: () => foo4
+    Test: () => Test,
+    foo: () => foo
 });
-var foo4 = 123;
+var foo = 123;
 console.log(a_exports, b_exports, c_exports, d_exports);

```