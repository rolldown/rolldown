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

//#region entry.ts
function foo(x = this) {
	console.log(this);
}
const objFoo = { foo(x = this) {
	console.log(this);
} };
class Foo {
	x = this;
	static y = this.z;
	foo(x = this) {
		console.log(this);
	}
	static bar(x = this) {
		console.log(this);
	}
}
new Foo(foo(objFoo));
if (nested) {
	function bar(x = this) {
		console.log(this);
	}
	const objBar = { foo(x = this) {
		console.log(this);
	} };
	class Bar {
		x = this;
		static y = this.z;
		foo(x = this) {
			console.log(this);
		}
		static bar(x = this) {
			console.log(this);
		}
	}
	new Bar(bar(objBar));
}

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -5,40 +5,31 @@
     foo(x = this) {
         console.log(this);
     }
 };
-var Foo = class {
-    constructor() {
-        this.x = this;
-    }
-    static {
-        this.y = this.z;
-    }
+class Foo {
+    x = this;
+    static y = this.z;
     foo(x = this) {
         console.log(this);
     }
     static bar(x = this) {
         console.log(this);
     }
-};
+}
 new Foo(foo(objFoo));
 if (nested) {
-    let bar = function (x = this) {
+    function bar(x = this) {
         console.log(this);
-    };
-    bar2 = bar;
+    }
     const objBar = {
         foo(x = this) {
             console.log(this);
         }
     };
     class Bar {
-        constructor() {
-            this.x = this;
-        }
-        static {
-            this.y = this.z;
-        }
+        x = this;
+        static y = this.z;
         foo(x = this) {
             console.log(this);
         }
         static bar(x = this) {
@@ -46,5 +37,4 @@
         }
     }
     new Bar(bar(objBar));
 }
-var bar2;

```