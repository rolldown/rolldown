# Reason
1. lowering ts experimental decorator
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

//#region entry.ts
var Foo = class {
	@dec(1) static prop1 = null;
	@dec(2) static prop2_ = null;
	@dec(3) static ["prop3"] = null;
	@dec(4) static ["prop4_"] = null;
	@dec(5) static ["prop5"] = null;
	@dec(6) static ["prop6_"] = null;
};

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,26 +1,10 @@
+
+//#region entry.ts
 var Foo = class {
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
+	@dec(1) static prop1 = null;
+	@dec(2) static prop2_ = null;
+	@dec(3) static ["prop3"] = null;
+	@dec(4) static ["prop4_"] = null;
+	@dec(5) static ["prop5"] = null;
+	@dec(6) static ["prop6_"] = null;
 };
-__decorateClass([dec(1)], Foo, "prop1", 2);
-__decorateClass([dec(2)], Foo, "a", 2);
-__decorateClass([dec(3)], Foo, "prop3", 2);
-__decorateClass([dec(4)], Foo, "prop4_", 2);
-__decorateClass([dec(5)], Foo, "prop5", 2);
-__decorateClass([dec(6)], Foo, "b", 2);

```