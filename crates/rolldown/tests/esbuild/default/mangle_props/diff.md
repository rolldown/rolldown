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

```
### diff
```diff
===================================================================
--- esbuild	/out/entry1.js
+++ rolldown	
@@ -1,36 +0,0 @@
-export function shouldMangle() {
-    let foo = {
-        a: 0,
-        b() {}
-    };
-    let {a: bar_} = foo;
-    ({a: bar_} = foo);
-    class foo_ {
-        a = 0;
-        b() {}
-        static a = 0;
-        static b() {}
-    }
-    return {
-        a: bar_,
-        c: foo_
-    };
-}
-export function shouldNotMangle() {
-    let foo = {
-        "bar_": 0,
-        "baz_"() {}
-    };
-    let {"bar_": bar_} = foo;
-    ({"bar_": bar_} = foo);
-    class foo_ {
-        "bar_" = 0;
-        "baz_"() {}
-        static "bar_" = 0;
-        static "baz_"() {}
-    }
-    return {
-        "bar_": bar_,
-        "foo_": foo_
-    };
-}

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

```
### diff
```diff
===================================================================
--- esbuild	/out/entry2.js
+++ rolldown	
@@ -1,4 +0,0 @@
-export default {
-    a: 0,
-    "baz_": 1
-};

```