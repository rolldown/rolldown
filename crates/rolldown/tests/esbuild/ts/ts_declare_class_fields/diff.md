# Diff
## /out.js
### esbuild
```js
// define-false/index.ts
() => null, c, () => null, C;
var Foo = class {
};
(() => new Foo())();

// define-true/index.ts
var _a;
var Bar = class {
  constructor() {
    __publicField(this, "a");
    __publicField(this, _a);
  }
  static A;
  static [(_a = (() => null, c), () => null, C)];
};
(() => new Bar())();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,13 +0,0 @@
-(() => null, c, () => null, C);
-var Foo = class {};
-(() => new Foo())();
-var _a;
-var Bar = class {
-    constructor() {
-        __publicField(this, "a");
-        __publicField(this, _a);
-    }
-    static A;
-    static [(_a = (() => null, c), () => null, C)];
-};
-(() => new Bar())();

```