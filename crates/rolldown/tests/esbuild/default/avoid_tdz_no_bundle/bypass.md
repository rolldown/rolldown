# Reason
1. We don't have no bundle mode, output should be same with esbuild bundle mode
# Diff
## /out.js
### esbuild
```js
class Foo {
  static foo = new Foo();
}
let foo = Foo.foo;
console.log(foo);
export class Bar {
}
export let bar = 123;
```
### rolldown
```js
//#region entry.js
var Foo = class Foo {
	static foo = new Foo();
};
let foo = Foo.foo;
console.log(foo);
var Bar = class {};
let bar = 123;

//#endregion
export { Bar, bar };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,8 @@
-class Foo {
+var Foo = class Foo {
     static foo = new Foo();
-}
-let foo = Foo.foo;
+};
+var foo = Foo.foo;
 console.log(foo);
-export class Bar {}
-export let bar = 123;
+var Bar = class {};
+var bar = 123;
+export {Bar, bar};

```