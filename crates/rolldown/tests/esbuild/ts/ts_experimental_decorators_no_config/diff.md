# Reason
1. ts experimental decorator
# Diff
## /out.js
### esbuild
```js
// entry.ts
var Foo = @x.y() @(new y.x()) class _Foo {
  @x @y mUndef;
  @x @y mDef = 1;
  @x @y method() {
    return new _Foo();
  }
  @x @y accessor aUndef;
  @x @y accessor aDef = 1;
  @x @y static sUndef;
  @x @y static sDef = new _Foo();
  @x @y static sMethod() {
    return new _Foo();
  }
  @x @y static accessor asUndef;
  @x @y static accessor asDef = 1;
  @x @y #mUndef;
  @x @y #mDef = 1;
  @x @y #method() {
    return new _Foo();
  }
  @x @y accessor #aUndef;
  @x @y accessor #aDef = 1;
  @x @y static #sUndef;
  @x @y static #sDef = 1;
  @x @y static #sMethod() {
    return new _Foo();
  }
  @x @y static accessor #asUndef;
  @x @y static accessor #asDef = 1;
};
export {
  Foo as default
};
```
### rolldown
```js

//#region entry.ts
var Foo = @x.y() @(new y.x()) class Foo {
	@x @y mUndef;
	@x @y mDef = 1;
	@x @y method() {
		return new Foo();
	}
	@x @y accessor aUndef;
	@x @y accessor aDef = 1;
	@x @y static sUndef;
	@x @y static sDef = new Foo();
	@x @y static sMethod() {
		return new Foo();
	}
	@x @y static accessor asUndef;
	@x @y static accessor asDef = 1;
	@x @y #mUndef;
	@x @y #mDef = 1;
	@x @y #method() {
		return new Foo();
	}
	@x @y accessor #aUndef;
	@x @y accessor #aDef = 1;
	@x @y static #sUndef;
	@x @y static #sDef = 1;
	@x @y static #sMethod() {
		return new Foo();
	}
	@x @y static accessor #asUndef;
	@x @y static accessor #asDef = 1;
};
//#endregion

export { Foo as default };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,34 +1,35 @@
-// entry.ts
-var Foo = @x.y() @(new y.x()) class _Foo {
-  @x @y mUndef;
-  @x @y mDef = 1;
-  @x @y method() {
-    return new _Foo();
-  }
-  @x @y accessor aUndef;
-  @x @y accessor aDef = 1;
-  @x @y static sUndef;
-  @x @y static sDef = new _Foo();
-  @x @y static sMethod() {
-    return new _Foo();
-  }
-  @x @y static accessor asUndef;
-  @x @y static accessor asDef = 1;
-  @x @y #mUndef;
-  @x @y #mDef = 1;
-  @x @y #method() {
-    return new _Foo();
-  }
-  @x @y accessor #aUndef;
-  @x @y accessor #aDef = 1;
-  @x @y static #sUndef;
-  @x @y static #sDef = 1;
-  @x @y static #sMethod() {
-    return new _Foo();
-  }
-  @x @y static accessor #asUndef;
-  @x @y static accessor #asDef = 1;
+
+//#region entry.ts
+var Foo = @x.y() @(new y.x()) class Foo {
+	@x @y mUndef;
+	@x @y mDef = 1;
+	@x @y method() {
+		return new Foo();
+	}
+	@x @y accessor aUndef;
+	@x @y accessor aDef = 1;
+	@x @y static sUndef;
+	@x @y static sDef = new Foo();
+	@x @y static sMethod() {
+		return new Foo();
+	}
+	@x @y static accessor asUndef;
+	@x @y static accessor asDef = 1;
+	@x @y #mUndef;
+	@x @y #mDef = 1;
+	@x @y #method() {
+		return new Foo();
+	}
+	@x @y accessor #aUndef;
+	@x @y accessor #aDef = 1;
+	@x @y static #sUndef;
+	@x @y static #sDef = 1;
+	@x @y static #sMethod() {
+		return new Foo();
+	}
+	@x @y static accessor #asUndef;
+	@x @y static accessor #asDef = 1;
 };
-export {
-  Foo as default
-};
\ No newline at end of file
+//#endregion
+
+export { Foo as default };
\ No newline at end of file

```