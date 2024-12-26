# Reason
1. could be done in minifier
# Diff
## /out/entry1.js
### esbuild
```js
export function shouldMangle_XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX() {
  let X = {
    X: 0,
    Y() {
    }
  }, { X: Y } = X;
  ({ X: Y } = X);
  class t {
    X = 0;
    Y() {
    }
    static X = 0;
    static Y() {
    }
  }
  return { X: Y, t };
}
export function shouldNotMangle_YYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYY() {
  let X = {
    bar_: 0,
    baz_() {
    }
  }, { bar_: Y } = X;
  ({ bar_: Y } = X);
  class t {
    bar_ = 0;
    baz_() {
    }
    static bar_ = 0;
    static baz_() {
    }
  }
  return { bar_: Y, foo_: t };
}
```
### rolldown
```js

//#region entry1.js
function shouldMangle_XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX() {
	let foo = {
		bar_: 0,
		baz_() {}
	};
	let { bar_ } = foo;
	({bar_} = foo);
	class foo_ {
		bar_ = 0;
		baz_() {}
		static bar_ = 0;
		static baz_() {}
	}
	return {
		bar_,
		foo_
	};
}
function shouldNotMangle_YYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYY() {
	let foo = {
		"bar_": 0,
		"baz_"() {}
	};
	let { "bar_": bar_ } = foo;
	({"bar_": bar_} = foo);
	class foo_ {
		"bar_" = 0;
		"baz_"() {}
		static "bar_" = 0;
		static "baz_"() {}
	}
	return {
		"bar_": bar_,
		"foo_": foo_
	};
}

//#endregion
export { shouldMangle_XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX, shouldNotMangle_YYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYY };
```
### diff
```diff
===================================================================
--- esbuild	/out/entry1.js
+++ rolldown	entry1.js
@@ -1,34 +1,37 @@
-export function shouldMangle_XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX() {
-    let X = {
-        X: 0,
-        Y() {}
-    }, {X: Y} = X;
-    ({X: Y} = X);
-    class t {
-        X = 0;
-        Y() {}
-        static X = 0;
-        static Y() {}
-    }
-    return {
-        X: Y,
-        t
-    };
-}
-export function shouldNotMangle_YYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYY() {
-    let X = {
+function shouldMangle_XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX() {
+    let foo = {
         bar_: 0,
         baz_() {}
-    }, {bar_: Y} = X;
-    ({bar_: Y} = X);
-    class t {
+    };
+    let {bar_} = foo;
+    ({bar_} = foo);
+    class foo_ {
         bar_ = 0;
         baz_() {}
         static bar_ = 0;
         static baz_() {}
     }
     return {
-        bar_: Y,
-        foo_: t
+        bar_,
+        foo_
     };
 }
+function shouldNotMangle_YYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYY() {
+    let foo = {
+        "bar_": 0,
+        "baz_"() {}
+    };
+    let {"bar_": bar_} = foo;
+    ({"bar_": bar_} = foo);
+    class foo_ {
+        "bar_" = 0;
+        "baz_"() {}
+        static "bar_" = 0;
+        static "baz_"() {}
+    }
+    return {
+        "bar_": bar_,
+        "foo_": foo_
+    };
+}
+export {shouldMangle_XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX, shouldNotMangle_YYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYY};

```
## /out/entry2.js
### esbuild
```js
export default {
  a: 0,
  baz_: 1
};
```
### rolldown
```js

//#region entry2.js
var entry2_default = {
	bar_: 0,
	"baz_": 1
};

//#endregion
export { entry2_default as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/entry2.js
+++ rolldown	entry2.js
@@ -1,4 +1,5 @@
-export default {
-    a: 0,
-    baz_: 1
+var entry2_default = {
+    bar_: 0,
+    "baz_": 1
 };
+export {entry2_default as default};

```