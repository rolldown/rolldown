# Diff
## /out.js
### esbuild
```js
// entry.ts
var Foo = class {
  static {
    this.prop1 = null;
  }
  static {
    this.a = null;
  }
  static {
    this["prop3"] = null;
  }
  static {
    this["prop4_"] = null;
  }
  static {
    this[/* @__KEY__ */ "prop5"] = null;
  }
  static {
    this.b = null;
  }
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
@@ -1,26 +0,0 @@
-var Foo = class {
-    static {
-        this.prop1 = null;
-    }
-    static {
-        this.a = null;
-    }
-    static {
-        this["prop3"] = null;
-    }
-    static {
-        this["prop4_"] = null;
-    }
-    static {
-        this["prop5"] = null;
-    }
-    static {
-        this.b = null;
-    }
-};
-__decorateClass([dec(1)], Foo, "prop1", 2);
-__decorateClass([dec(2)], Foo, "a", 2);
-__decorateClass([dec(3)], Foo, "prop3", 2);
-__decorateClass([dec(4)], Foo, "prop4_", 2);
-__decorateClass([dec(5)], Foo, "prop5", 2);
-__decorateClass([dec(6)], Foo, "b", 2);

```