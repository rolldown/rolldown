# Diff
## /out.js
### esbuild
```js
// entry.ts
var Foo = class {
};
__name(Foo, "Foo");
Foo = __decorateClass([
  decoratorMustComeAfterName
], Foo);
export {
  Foo
};
```
### rolldown
```js

//#region entry.ts
@decoratorMustComeAfterName class Foo {}

//#endregion
export { Foo };

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,6 @@
-var Foo = class {};
-__name(Foo, "Foo");
-Foo = __decorateClass([decoratorMustComeAfterName], Foo);
-export {Foo};
+
+//#region entry.ts
+@decoratorMustComeAfterName class Foo {}
+
+//#endregion
+export { Foo };

```