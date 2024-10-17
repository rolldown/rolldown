# Diff
## /out.js
### esbuild
```js
// b.ts
var Foo = class {
};
((Foo2) => {
  Foo2.foo = 1;
})(Foo || (Foo = {}));
((Foo2) => {
  Foo2.bar = 2;
})(Foo || (Foo = {}));

// a.ts
console.log(new Foo());
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
-var Foo = class {};
-(Foo2 => {
-    Foo2.foo = 1;
-})(Foo || (Foo = {}));
-(Foo2 => {
-    Foo2.bar = 2;
-})(Foo || (Foo = {}));
-console.log(new Foo());

```