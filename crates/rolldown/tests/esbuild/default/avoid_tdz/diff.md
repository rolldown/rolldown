# Diff
## /out.js
### esbuild
```js
// entry.js
var Foo = class _Foo {
  static foo = new _Foo();
};
var foo = Foo.foo;
console.log(foo);
var Bar = class {
};
var bar = 123;
export {
  Bar,
  bar
};
```
### rolldown
```js
import { default as assert } from "node:assert";

//#region entry.js
class Foo {
	static foo = new Foo();
}
let foo = Foo.foo;
assert(foo instanceof Foo);
class Bar {}
let bar = 123;

//#endregion
export { Bar, bar };

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,8 @@
-var Foo = class _Foo {
-    static foo = new _Foo();
-};
+class Foo {
+    static foo = new Foo();
+}
 var foo = Foo.foo;
-console.log(foo);
-var Bar = class {};
+assert(foo instanceof Foo);
+class Bar {}
 var bar = 123;
 export {Bar, bar};

```