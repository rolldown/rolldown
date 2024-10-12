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

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,4 +0,0 @@
-var Foo = class {};
-__name(Foo, "Foo");
-Foo = __decorateClass([decoratorMustComeAfterName], Foo);
-export {Foo};

```