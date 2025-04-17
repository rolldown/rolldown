# Reason
1. rename private identifier
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

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,20 +1,20 @@
-class Foo {
+var Foo = class {
     #foo;
     foo = class {
+        #foo;
         #foo2;
-        #foo22;
-        #bar2;
+        #bar;
     };
     get #bar() {}
     set #bar(x) {}
-}
-class Bar {
+};
+var Bar = class {
     #foo;
     foo = class {
         #foo2;
-        #foo3;
-        #bar2;
+        #foo;
+        #bar;
     };
     get #bar() {}
     set #bar(x) {}
-}
+};

```