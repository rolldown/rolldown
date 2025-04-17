# Reason
1. should not convert `ClassDeclaration` to `ClassExpr`
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
    x = this;
    static y = this.z;
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

//#region entry.ts
function foo(x = this) {
	console.log(this);
}
const objFoo = { foo(x = this) {
	console.log(this);
} };
var Foo = class {
	x = this;
	static y = this.z;
	foo(x = this) {
		console.log(this);
	}
	static bar(x = this) {
		console.log(this);
	}
};
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

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,28 +1,27 @@
 function foo(x = this) {
     console.log(this);
 }
-const objFoo = {
+var objFoo = {
     foo(x = this) {
         console.log(this);
     }
 };
-class Foo {
+var Foo = class {
     x = this;
     static y = this.z;
     foo(x = this) {
         console.log(this);
     }
     static bar(x = this) {
         console.log(this);
     }
-}
+};
 new Foo(foo(objFoo));
 if (nested) {
-    let bar2 = function (x = this) {
+    function bar(x = this) {
         console.log(this);
-    };
-    var bar = bar2;
+    }
     const objBar = {
         foo(x = this) {
             console.log(this);
         }
@@ -36,6 +35,6 @@
         static bar(x = this) {
             console.log(this);
         }
     }
-    new Bar(bar2(objBar));
+    new Bar(bar(objBar));
 }

```