# Diff
## /out.js
### esbuild
```js
// entry.ts
var Foo = class {
  static prop1 = null;
  static a = null;
  static ["prop3"] = null;
  static ["prop4_"] = null;
  static [/* @__KEY__ */ "prop5"] = null;
  static [/* @__KEY__ */ "b"] = null;
};
__decorateClass([
  dec(1)
], Foo, "prop1", 2);
__decorateClass([
  dec(2)
], Foo, "a", 2);
__decorateClass([
  dec(3)
], Foo, "prop3", 2);
__decorateClass([
  dec(4)
], Foo, "prop4_", 2);
__decorateClass([
  dec(5)
], Foo, /* @__KEY__ */ "prop5", 2);
__decorateClass([
  dec(6)
], Foo, /* @__KEY__ */ "b", 2);
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
-    static prop1 = null;
-    static a = null;
-    static ["prop3"] = null;
-    static ["prop4_"] = null;
-    static ["prop5"] = null;
-    static ["b"] = null;
-};
-__decorateClass([dec(1)], Foo, "prop1", 2);
-__decorateClass([dec(2)], Foo, "a", 2);
-__decorateClass([dec(3)], Foo, "prop3", 2);
-__decorateClass([dec(4)], Foo, "prop4_", 2);
-__decorateClass([dec(5)], Foo, "prop5", 2);
-__decorateClass([dec(6)], Foo, "b", 2);

```