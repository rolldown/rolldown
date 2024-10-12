# Diff
## /out.js
### esbuild
```js
class Foo {
  #foo;
  foo = class {
    #foo2;
    #foo22;
    #bar2;
  };
  get #bar() {
  }
  set #bar(x) {
  }
}
class Bar {
  #foo;
  foo = class {
    #foo2;
    #foo3;
    #bar2;
  };
  get #bar() {
  }
  set #bar(x) {
  }
}
```
### rolldown
```js


```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,20 +0,0 @@
-class Foo {
-    #foo;
-    foo = class {
-        #foo2;
-        #foo22;
-        #bar2;
-    };
-    get #bar() {}
-    set #bar(x) {}
-}
-class Bar {
-    #foo;
-    foo = class {
-        #foo2;
-        #foo3;
-        #bar2;
-    };
-    get #bar() {}
-    set #bar(x) {}
-}

```