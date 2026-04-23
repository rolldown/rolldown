## /out.js
### esbuild
```js
// entry.ts
var Foo = class {
  constructor(b1 = 2.1, b2 = 2.2) {
    this.b1 = b1;
    this.b2 = b2;
  }
  b1;
  b2;
  static {
    console.log("a");
  }
  a = 1;
  static {
    console.log("b");
  }
  static {
    console.log("c");
  }
  c = 3;
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
@@ -1,19 +0,0 @@
-var Foo = class {
-    constructor(b1 = 2.1, b2 = 2.2) {
-        this.b1 = b1;
-        this.b2 = b2;
-    }
-    b1;
-    b2;
-    static {
-        console.log("a");
-    }
-    a = 1;
-    static {
-        console.log("b");
-    }
-    static {
-        console.log("c");
-    }
-    c = 3;
-};

```