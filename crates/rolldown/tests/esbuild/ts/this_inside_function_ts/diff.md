# Diff
## /out.js
### esbuild
```js
// entry.ts
function foo(x = this) {
  console.log(this);
}
var objFoo = {
  foo(x = this) {
    console.log(this);
  }
};
var Foo = class {
  constructor() {
    this.x = this;
  }
  static {
    this.y = this.z;
  }
  foo(x = this) {
    console.log(this);
  }
  static bar(x = this) {
    console.log(this);
  }
};
new Foo(foo(objFoo));
if (nested) {
  let bar = function(x = this) {
    console.log(this);
  };
  bar2 = bar;
  const objBar = {
    foo(x = this) {
      console.log(this);
    }
  };
  class Bar {
    constructor() {
      this.x = this;
    }
    static {
      this.y = this.z;
    }
    foo(x = this) {
      console.log(this);
    }
    static bar(x = this) {
      console.log(this);
    }
  }
  new Bar(bar(objBar));
}
var bar2;
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,50 +0,0 @@
-function foo(x = this) {
-    console.log(this);
-}
-var objFoo = {
-    foo(x = this) {
-        console.log(this);
-    }
-};
-var Foo = class {
-    constructor() {
-        this.x = this;
-    }
-    static {
-        this.y = this.z;
-    }
-    foo(x = this) {
-        console.log(this);
-    }
-    static bar(x = this) {
-        console.log(this);
-    }
-};
-new Foo(foo(objFoo));
-if (nested) {
-    let bar = function (x = this) {
-        console.log(this);
-    };
-    bar2 = bar;
-    const objBar = {
-        foo(x = this) {
-            console.log(this);
-        }
-    };
-    class Bar {
-        constructor() {
-            this.x = this;
-        }
-        static {
-            this.y = this.z;
-        }
-        foo(x = this) {
-            console.log(this);
-        }
-        static bar(x = this) {
-            console.log(this);
-        }
-    }
-    new Bar(bar(objBar));
-}
-var bar2;

```