# Diff
## /out.js
### esbuild
```js
// entry.ts
var Foo = class {
  constructor() {
    this.prop1 = null;
    this.a = null;
    this["prop3"] = null;
    this["prop4_"] = null;
    this[/* @__KEY__ */ "prop5"] = null;
    this.b = null;
  }
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
@@ -1,16 +0,0 @@
-var Foo = class {
-    constructor() {
-        this.prop1 = null;
-        this.a = null;
-        this["prop3"] = null;
-        this["prop4_"] = null;
-        this["prop5"] = null;
-        this.b = null;
-    }
-};
-__decorateClass([dec(1)], Foo.prototype, "prop1", 2);
-__decorateClass([dec(2)], Foo.prototype, "a", 2);
-__decorateClass([dec(3)], Foo.prototype, "prop3", 2);
-__decorateClass([dec(4)], Foo.prototype, "prop4_", 2);
-__decorateClass([dec(5)], Foo.prototype, "prop5", 2);
-__decorateClass([dec(6)], Foo.prototype, "b", 2);

```