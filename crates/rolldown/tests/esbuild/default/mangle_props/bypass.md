# Reason
1. could be done in minifier
# Diff
## /out/entry1.js
### esbuild
```js
export function shouldMangle() {
  let foo = {
    a: 0,
    b() {
    }
  };
  let { a: bar_ } = foo;
  ({ a: bar_ } = foo);
  class foo_ {
    a = 0;
    b() {
    }
    static a = 0;
    static b() {
    }
  }
  return { a: bar_, c: foo_ };
}
export function shouldNotMangle() {
  let foo = {
    "bar_": 0,
    "baz_"() {
    }
  };
  let { "bar_": bar_ } = foo;
  ({ "bar_": bar_ } = foo);
  class foo_ {
    "bar_" = 0;
    "baz_"() {
    }
    static "bar_" = 0;
    static "baz_"() {
    }
  }
  return { "bar_": bar_, "foo_": foo_ };
}
```
### rolldown
```js

//#region entry1.js
function shouldMangle() {
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
function shouldNotMangle() {
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
export { shouldMangle, shouldNotMangle };
```
### diff
```diff
===================================================================
--- esbuild	/out/entry1.js
+++ rolldown	entry1.js
@@ -1,23 +1,23 @@
-export function shouldMangle() {
+function shouldMangle() {
     let foo = {
-        a: 0,
-        b() {}
+        bar_: 0,
+        baz_() {}
     };
-    let {a: bar_} = foo;
-    ({a: bar_} = foo);
+    let {bar_} = foo;
+    ({bar_} = foo);
     class foo_ {
-        a = 0;
-        b() {}
-        static a = 0;
-        static b() {}
+        bar_ = 0;
+        baz_() {}
+        static bar_ = 0;
+        static baz_() {}
     }
     return {
-        a: bar_,
-        c: foo_
+        bar_,
+        foo_
     };
 }
-export function shouldNotMangle() {
+function shouldNotMangle() {
     let foo = {
         "bar_": 0,
         "baz_"() {}
     };
@@ -33,4 +33,5 @@
         "bar_": bar_,
         "foo_": foo_
     };
 }
+export {shouldMangle, shouldNotMangle};

```
## /out/entry2.js
### esbuild
```js
export default {
  a: 0,
  "baz_": 1
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
+var entry2_default = {
+    bar_: 0,
     "baz_": 1
 };
+export {entry2_default as default};

```