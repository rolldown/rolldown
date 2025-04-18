# Reason
1. could be done in minifier
# Diff
## /out.js
### esbuild
```js
class Foo {
  #a;
  foo = class {
    #s;
    #f;
    #r;
  };
  get #o() {
  }
  set #o(a) {
  }
}
class Bar {
  #a;
  foo = class {
    #s;
    #f;
    #r;
  };
  get #o() {
  }
  set #o(a) {
  }
}
```
### rolldown
```js
//#region entry.js
var Foo = class {
	#foo;
	foo = class {
		#foo;
		#foo2;
		#bar;
	};
	get #bar() {}
	set #bar(x) {}
};
var Bar = class {
	#foo;
	foo = class {
		#foo2;
		#foo;
		#bar;
	};
	get #bar() {}
	set #bar(x) {}
};

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,20 +1,20 @@
-class Foo {
-    #a;
+var Foo = class {
+    #foo;
     foo = class {
-        #s;
-        #f;
-        #r;
+        #foo;
+        #foo2;
+        #bar;
     };
-    get #o() {}
-    set #o(a) {}
-}
-class Bar {
-    #a;
+    get #bar() {}
+    set #bar(x) {}
+};
+var Bar = class {
+    #foo;
     foo = class {
-        #s;
-        #f;
-        #r;
+        #foo2;
+        #foo;
+        #bar;
     };
-    get #o() {}
-    set #o(a) {}
-}
+    get #bar() {}
+    set #bar(x) {}
+};

```