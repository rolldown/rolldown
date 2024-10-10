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

```
### diff
```diff
===================================================================
--- esbuild	/out/entry1.js
+++ rolldown	
@@ -1,34 +0,0 @@
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
-        bar_: 0,
-        baz_() {}
-    }, {bar_: Y} = X;
-    ({bar_: Y} = X);
-    class t {
-        bar_ = 0;
-        baz_() {}
-        static bar_ = 0;
-        static baz_() {}
-    }
-    return {
-        bar_: Y,
-        foo_: t
-    };
-}

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

```
### diff
```diff
===================================================================
--- esbuild	/out/entry2.js
+++ rolldown	
@@ -1,4 +0,0 @@
-export default {
-    a: 0,
-    baz_: 1
-};

```