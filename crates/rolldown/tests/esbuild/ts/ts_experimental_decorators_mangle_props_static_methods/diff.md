# Diff
## /out.js
### esbuild
```js
// entry.ts
var Foo = class {
  static prop1() {
  }
  static a() {
  }
  static ["prop3"]() {
  }
  static ["prop4_"]() {
  }
  static [/* @__KEY__ */ "prop5"]() {
  }
  static [/* @__KEY__ */ "b"]() {
  }
};
__decorateClass([
  dec(1)
], Foo, "prop1", 1);
__decorateClass([
  dec(2)
], Foo, "a", 1);
__decorateClass([
  dec(3)
], Foo, "prop3", 1);
__decorateClass([
  dec(4)
], Foo, "prop4_", 1);
__decorateClass([
  dec(5)
], Foo, /* @__KEY__ */ "prop5", 1);
__decorateClass([
  dec(6)
], Foo, /* @__KEY__ */ "b", 1);
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
-    static prop1() {}
-    static a() {}
-    static ["prop3"]() {}
-    static ["prop4_"]() {}
-    static ["prop5"]() {}
-    static ["b"]() {}
-};
-__decorateClass([dec(1)], Foo, "prop1", 1);
-__decorateClass([dec(2)], Foo, "a", 1);
-__decorateClass([dec(3)], Foo, "prop3", 1);
-__decorateClass([dec(4)], Foo, "prop4_", 1);
-__decorateClass([dec(5)], Foo, "prop5", 1);
-__decorateClass([dec(6)], Foo, "b", 1);

```