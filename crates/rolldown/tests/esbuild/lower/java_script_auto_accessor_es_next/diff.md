# Diff
## /out/js-define.js
### esbuild
```js
class Foo {
  accessor one = 1;
  accessor #two = 2;
  accessor [three()] = 3;
  static accessor four = 4;
  static accessor #five = 5;
  static accessor [six()] = 6;
}
```
### rolldown
```js

//#region js-define.js
var Foo = class {
	accessor one = 1;
	accessor #two = 2;
	accessor [three()] = 3;
	static accessor four = 4;
	static accessor #five = 5;
	static accessor [six()] = 6;
};
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/js-define.js
+++ rolldown	js-define.js
@@ -1,8 +1,11 @@
-class Foo {
-  accessor one = 1;
-  accessor #two = 2;
-  accessor [three()] = 3;
-  static accessor four = 4;
-  static accessor #five = 5;
-  static accessor [six()] = 6;
-}
\ No newline at end of file
+
+//#region js-define.js
+var Foo = class {
+	accessor one = 1;
+	accessor #two = 2;
+	accessor [three()] = 3;
+	static accessor four = 4;
+	static accessor #five = 5;
+	static accessor [six()] = 6;
+};
+//#endregion

```
## /out/ts-define/ts-define.js
### esbuild
```js
class Foo {
  accessor one = 1;
  accessor #two = 2;
  accessor [three()] = 3;
  static accessor four = 4;
  static accessor #five = 5;
  static accessor [six()] = 6;
}
class Normal {
  accessor a = b;
  c = d;
}
class Private {
  accessor #a = b;
  c = d;
}
class StaticNormal {
  static accessor a = b;
  static c = d;
}
class StaticPrivate {
  static accessor #a = b;
  static c = d;
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/ts-define/ts-define.js
+++ rolldown	
@@ -1,24 +0,0 @@
-class Foo {
-  accessor one = 1;
-  accessor #two = 2;
-  accessor [three()] = 3;
-  static accessor four = 4;
-  static accessor #five = 5;
-  static accessor [six()] = 6;
-}
-class Normal {
-  accessor a = b;
-  c = d;
-}
-class Private {
-  accessor #a = b;
-  c = d;
-}
-class StaticNormal {
-  static accessor a = b;
-  static c = d;
-}
-class StaticPrivate {
-  static accessor #a = b;
-  static c = d;
-}
\ No newline at end of file

```
## /out/ts-assign/ts-assign.js
### esbuild
```js
var _a, __a;
class Foo {
  accessor one = 1;
  accessor #two = 2;
  accessor [three()] = 3;
  static accessor four = 4;
  static accessor #five = 5;
  static accessor [six()] = 6;
}
class Normal {
  constructor() {
    __privateAdd(this, _a, b);
    this.c = d;
  }
  get a() {
    return __privateGet(this, _a);
  }
  set a(_) {
    __privateSet(this, _a, _);
  }
}
_a = new WeakMap();
class Private {
  constructor() {
    __privateAdd(this, __a, b);
    this.c = d;
  }
  get #a() {
    return __privateGet(this, __a);
  }
  set #a(_) {
    __privateSet(this, __a, _);
  }
}
__a = new WeakMap();
class StaticNormal {
  static accessor a = b;
  static {
    this.c = d;
  }
}
class StaticPrivate {
  static accessor #a = b;
  static {
    this.c = d;
  }
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/ts-assign/ts-assign.js
+++ rolldown	
@@ -1,47 +0,0 @@
-var _a, __a;
-class Foo {
-  accessor one = 1;
-  accessor #two = 2;
-  accessor [three()] = 3;
-  static accessor four = 4;
-  static accessor #five = 5;
-  static accessor [six()] = 6;
-}
-class Normal {
-  constructor() {
-    __privateAdd(this, _a, b);
-    this.c = d;
-  }
-  get a() {
-    return __privateGet(this, _a);
-  }
-  set a(_) {
-    __privateSet(this, _a, _);
-  }
-}
-_a = new WeakMap();
-class Private {
-  constructor() {
-    __privateAdd(this, __a, b);
-    this.c = d;
-  }
-  get #a() {
-    return __privateGet(this, __a);
-  }
-  set #a(_) {
-    __privateSet(this, __a, _);
-  }
-}
-__a = new WeakMap();
-class StaticNormal {
-  static accessor a = b;
-  static {
-    this.c = d;
-  }
-}
-class StaticPrivate {
-  static accessor #a = b;
-  static {
-    this.c = d;
-  }
-}
\ No newline at end of file

```