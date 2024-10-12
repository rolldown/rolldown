# Diff
## /out.js
### esbuild
```js
function foo(x = this) {
  console.log(this);
}
const objFoo = {
  foo(x = this) {
    console.log(this);
  }
};
class Foo {
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
new Foo(foo(objFoo));
if (nested) {
  let bar2 = function(x = this) {
    console.log(this);
  };
  var bar = bar2;
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
  new Bar(bar2(objBar));
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,49 +0,0 @@
-function foo(x = this) {
-    console.log(this);
-}
-const objFoo = {
-    foo(x = this) {
-        console.log(this);
-    }
-};
-class Foo {
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
-}
-new Foo(foo(objFoo));
-if (nested) {
-    let bar2 = function (x = this) {
-        console.log(this);
-    };
-    var bar = bar2;
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
-    new Bar(bar2(objBar));
-}

```