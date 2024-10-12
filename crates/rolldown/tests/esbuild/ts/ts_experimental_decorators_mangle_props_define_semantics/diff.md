# Diff
## /out.js
### esbuild
```js
// entry.ts
var Foo = class {
  prop1 = null;
  a = null;
  ["prop3"] = null;
  ["prop4_"] = null;
  [/* @__KEY__ */ "prop5"] = null;
  [/* @__KEY__ */ "b"] = null;
};
__decorateClass([
  dec(1)
], Foo.prototype, "prop1", 2);
__decorateClass([
  dec(2)
], Foo.prototype, "a", 2);
__decorateClass([
  dec(3)
], Foo.prototype, "prop3", 2);
__decorateClass([
  dec(4)
], Foo.prototype, "prop4_", 2);
__decorateClass([
  dec(5)
], Foo.prototype, /* @__KEY__ */ "prop5", 2);
__decorateClass([
  dec(6)
], Foo.prototype, /* @__KEY__ */ "b", 2);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,14 +0,0 @@
-var Foo = class {
-    prop1 = null;
-    a = null;
-    ["prop3"] = null;
-    ["prop4_"] = null;
-    ["prop5"] = null;
-    ["b"] = null;
-};
-__decorateClass([dec(1)], Foo.prototype, "prop1", 2);
-__decorateClass([dec(2)], Foo.prototype, "a", 2);
-__decorateClass([dec(3)], Foo.prototype, "prop3", 2);
-__decorateClass([dec(4)], Foo.prototype, "prop4_", 2);
-__decorateClass([dec(5)], Foo.prototype, "prop5", 2);
-__decorateClass([dec(6)], Foo.prototype, "b", 2);

```