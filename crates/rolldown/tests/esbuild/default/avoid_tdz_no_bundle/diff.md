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
class Foo {
	static foo = new Foo();
}
let foo = Foo.foo;
console.log(foo);
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
@@ -1,7 +1,8 @@
 class Foo {
     static foo = new Foo();
 }
-let foo = Foo.foo;
+var foo = Foo.foo;
 console.log(foo);
-export class Bar {}
-export let bar = 123;
+class Bar {}
+var bar = 123;
+export {Bar, bar};

```