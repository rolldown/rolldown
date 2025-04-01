# Reason
1. lowering ts experimental decorator
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

//#region entry.ts
var Foo = class {
	@dec(1) static prop1() {}
	@dec(2) static prop2_() {}
	@dec(3) static ["prop3"]() {}
	@dec(4) static ["prop4_"]() {}
	@dec(5) static ["prop5"]() {}
	@dec(6) static ["prop6_"]() {}
};
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,14 +1,11 @@
+
+//#region entry.ts
 var Foo = class {
-    static prop1() {}
-    static a() {}
-    static ["prop3"]() {}
-    static ["prop4_"]() {}
-    static ["prop5"]() {}
-    static ["b"]() {}
+	@dec(1) static prop1() {}
+	@dec(2) static prop2_() {}
+	@dec(3) static ["prop3"]() {}
+	@dec(4) static ["prop4_"]() {}
+	@dec(5) static ["prop5"]() {}
+	@dec(6) static ["prop6_"]() {}
 };
-__decorateClass([dec(1)], Foo, "prop1", 1);
-__decorateClass([dec(2)], Foo, "a", 1);
-__decorateClass([dec(3)], Foo, "prop3", 1);
-__decorateClass([dec(4)], Foo, "prop4_", 1);
-__decorateClass([dec(5)], Foo, "prop5", 1);
-__decorateClass([dec(6)], Foo, "b", 1);
+//#endregion

```