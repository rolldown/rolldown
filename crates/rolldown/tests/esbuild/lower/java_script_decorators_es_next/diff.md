# Diff
## /out.js
### esbuild
```js
@x.y()
@(new y.x())
export default class Foo {
  @x @y mUndef;
  @x @y mDef = 1;
  @x @y method() {
    return new Foo();
  }
  @x @y static sUndef;
  @x @y static sDef = new Foo();
  @x @y static sMethod() {
    return new Foo();
  }
}
```
### rolldown
```js

//#region entry.js
var Foo = @x.y() @(new y.x()) class Foo {
	@x @y mUndef;
	@x @y mDef = 1;
	@x @y method() {
		return new Foo();
	}
	@x @y static sUndef;
	@x @y static sDef = new Foo();
	@x @y static sMethod() {
		return new Foo();
	}
};

export { Foo as default };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,14 +1,16 @@
-@x.y()
-@(new y.x())
-export default class Foo {
-  @x @y mUndef;
-  @x @y mDef = 1;
-  @x @y method() {
-    return new Foo();
-  }
-  @x @y static sUndef;
-  @x @y static sDef = new Foo();
-  @x @y static sMethod() {
-    return new Foo();
-  }
-}
\ No newline at end of file
+
+//#region entry.js
+var Foo = @x.y() @(new y.x()) class Foo {
+	@x @y mUndef;
+	@x @y mDef = 1;
+	@x @y method() {
+		return new Foo();
+	}
+	@x @y static sUndef;
+	@x @y static sDef = new Foo();
+	@x @y static sMethod() {
+		return new Foo();
+	}
+};
+
+export { Foo as default };
\ No newline at end of file

```