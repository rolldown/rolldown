# Reason
1. should have same output in bundle mode, https://hyrious.me/esbuild-repl/?version=0.23.0&b=e%00entry.ts%00const+keepThisToo+%3D+Symbol%28%27keepThisToo%27%29%0Adeclare+const+REMOVE_THIS_TOO%3A+unique+symbol%0Aabstract+class+Foo+%7B%0A%09keepThis%3A+any%0A%09%5BkeepThisToo%5D%3A+any%0A%09abstract+REMOVE_THIS%3A+any%0A%09abstract+%5BREMOVE_THIS_TOO%5D%3A+any%0A%09abstract+%5B%28x+%3D%3E+y+%3D%3E+x+%2B+y%29%28%27nested%27%29%28%27scopes%27%29%5D%3A+any%0A%7D%0A%28%28%29+%3D%3E+new+Foo%28%29%29%28%29%0A&b=%00file.js%00&o=%7B%0A++treeShaking%3A+true%2C%0A++external%3A+%5B%22c%22%2C+%22a%22%2C+%22b%22%5D%2C%0A%22bundle%22%3A+true%2C%0Aformat%3A+%22esm%22%0A%7D
# Diff
## /out.js
### esbuild
```js
const keepThisToo = Symbol("keepThisToo");
class Foo {
  keepThis;
  [keepThisToo];
}
(() => new Foo())();
```
### rolldown
```js

//#region entry.ts
const keepThisToo = Symbol("keepThisToo");
var Foo = class {
	keepThis;
	[keepThisToo];
};
new Foo();
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,6 @@
 const keepThisToo = Symbol("keepThisToo");
-class Foo {
+var Foo = class {
     keepThis;
     [keepThisToo];
-}
-(() => new Foo())();
+};
+new Foo();

```