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

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,8 +0,0 @@
-var Foo = class _Foo {
-    static foo = new _Foo();
-};
-var foo = Foo.foo;
-console.log(foo);
-var Bar = class {};
-var bar = 123;
-export {Bar, bar};

```