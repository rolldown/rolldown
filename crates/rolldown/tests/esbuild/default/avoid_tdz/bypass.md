# Reason
1. Different naming style
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
import assert from "node:assert";

//#region entry.js
var Foo = class Foo {
	static foo = new Foo();
};
let foo = Foo.foo;
assert(foo instanceof Foo, true);
var Bar = class {};
let bar = 123;

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
+var Foo = class Foo {
+    static foo = new Foo();
 };
 var foo = Foo.foo;
-console.log(foo);
+assert(foo instanceof Foo, true);
 var Bar = class {};
 var bar = 123;
 export {Bar, bar};

```