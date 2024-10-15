# Reason
1. not align
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
class Foo {
	#foo;
	foo = class {
		#foo;
		#foo2;
		#bar;
	};
	get #bar() {}
	set #bar(x) {}
}
class Bar {
	#foo;
	foo = class {
		#foo2;
		#foo;
		#bar;
	};
	get #bar() {}
	set #bar(x) {}
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,20 +1,20 @@
 class Foo {
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
 }
 class Bar {
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
 }

```