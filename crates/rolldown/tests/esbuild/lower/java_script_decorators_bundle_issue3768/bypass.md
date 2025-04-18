# Reason
1. pure transformation is handled by `oxc-transform`
# Diff
## /out/base-instance-accessor.js
### esbuild
```js
// base-instance-accessor.js
var _foo_dec, _init, _foo;
_foo_dec = [dec];
var _Foo = class _Foo {
  constructor() {
    __privateAdd(this, _foo, __runInitializers(_init, 8, this, _Foo)), __runInitializers(_init, 11, this);
  }
};
_init = __decoratorStart(null);
_foo = new WeakMap();
__decorateElement(_init, 4, "foo", _foo_dec, _Foo, _foo);
__decoratorMetadata(_init, _Foo);
var Foo = _Foo;
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/base-instance-accessor.js
+++ rolldown	base-instance-accessor.js
@@ -1,12 +0,0 @@
-var _foo_dec, _init, _foo;
-_foo_dec = [dec];
-var _Foo = class _Foo {
-    constructor() {
-        (__privateAdd(this, _foo, __runInitializers(_init, 8, this, _Foo)), __runInitializers(_init, 11, this));
-    }
-};
-_init = __decoratorStart(null);
-_foo = new WeakMap();
-__decorateElement(_init, 4, "foo", _foo_dec, _Foo, _foo);
-__decoratorMetadata(_init, _Foo);
-var Foo = _Foo;

```
## /out/base-instance-field.js
### esbuild
```js
// base-instance-field.js
var _foo_dec, _init;
_foo_dec = [dec];
var _Foo = class _Foo {
  constructor() {
    __publicField(this, "foo", __runInitializers(_init, 8, this, _Foo)), __runInitializers(_init, 11, this);
  }
};
_init = __decoratorStart(null);
__decorateElement(_init, 5, "foo", _foo_dec, _Foo);
__decoratorMetadata(_init, _Foo);
var Foo = _Foo;
```
### rolldown
```js
//#region base-instance-field.js
var Foo = class Foo {
	@dec foo = Foo;
};

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/base-instance-field.js
+++ rolldown	base-instance-field.js
@@ -1,11 +1,6 @@
-var _foo_dec, _init;
-_foo_dec = [dec];
-var _Foo = class _Foo {
-    constructor() {
-        (__publicField(this, "foo", __runInitializers(_init, 8, this, _Foo)), __runInitializers(_init, 11, this));
-    }
+//#region base-instance-field.js
+var Foo = class Foo {
+	@dec foo = Foo;
 };
-_init = __decoratorStart(null);
-__decorateElement(_init, 5, "foo", _foo_dec, _Foo);
-__decoratorMetadata(_init, _Foo);
-var Foo = _Foo;
+
+//#endregion
\ No newline at end of file

```
## /out/base-instance-method.js
### esbuild
```js
// base-instance-method.js
var _foo_dec, _init;
_foo_dec = [dec];
var _Foo = class _Foo {
  constructor() {
    __runInitializers(_init, 5, this);
  }
  foo() {
    return _Foo;
  }
};
_init = __decoratorStart(null);
__decorateElement(_init, 1, "foo", _foo_dec, _Foo);
__decoratorMetadata(_init, _Foo);
var Foo = _Foo;
```
### rolldown
```js
//#region base-instance-method.js
var Foo = class Foo {
	@dec foo() {
		return Foo;
	}
};

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/base-instance-method.js
+++ rolldown	base-instance-method.js
@@ -1,14 +1,8 @@
-var _foo_dec, _init;
-_foo_dec = [dec];
-var _Foo = class _Foo {
-    constructor() {
-        __runInitializers(_init, 5, this);
-    }
-    foo() {
-        return _Foo;
-    }
+//#region base-instance-method.js
+var Foo = class Foo {
+	@dec foo() {
+		return Foo;
+	}
 };
-_init = __decoratorStart(null);
-__decorateElement(_init, 1, "foo", _foo_dec, _Foo);
-__decoratorMetadata(_init, _Foo);
-var Foo = _Foo;
+
+//#endregion
\ No newline at end of file

```
## /out/base-static-accessor.js
### esbuild
```js
// base-static-accessor.js
var _foo_dec, _init, _foo;
_foo_dec = [dec];
var _Foo = class _Foo {
};
_init = __decoratorStart(null);
_foo = new WeakMap();
__decorateElement(_init, 12, "foo", _foo_dec, _Foo, _foo);
__decoratorMetadata(_init, _Foo);
__privateAdd(_Foo, _foo, __runInitializers(_init, 8, _Foo, _Foo)), __runInitializers(_init, 11, _Foo);
var Foo = _Foo;
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/base-static-accessor.js
+++ rolldown	base-static-accessor.js
@@ -1,9 +0,0 @@
-var _foo_dec, _init, _foo;
-_foo_dec = [dec];
-var _Foo = class _Foo {};
-_init = __decoratorStart(null);
-_foo = new WeakMap();
-__decorateElement(_init, 12, "foo", _foo_dec, _Foo, _foo);
-__decoratorMetadata(_init, _Foo);
-(__privateAdd(_Foo, _foo, __runInitializers(_init, 8, _Foo, _Foo)), __runInitializers(_init, 11, _Foo));
-var Foo = _Foo;

```
## /out/base-static-field.js
### esbuild
```js
// base-static-field.js
var _foo_dec, _init;
_foo_dec = [dec];
var _Foo = class _Foo {
};
_init = __decoratorStart(null);
__decorateElement(_init, 13, "foo", _foo_dec, _Foo);
__decoratorMetadata(_init, _Foo);
__publicField(_Foo, "foo", __runInitializers(_init, 8, _Foo, _Foo)), __runInitializers(_init, 11, _Foo);
var Foo = _Foo;
```
### rolldown
```js
//#region base-static-field.js
var Foo = class Foo {
	@dec static foo = Foo;
};

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/base-static-field.js
+++ rolldown	base-static-field.js
@@ -1,8 +1,6 @@
-var _foo_dec, _init;
-_foo_dec = [dec];
-var _Foo = class _Foo {};
-_init = __decoratorStart(null);
-__decorateElement(_init, 13, "foo", _foo_dec, _Foo);
-__decoratorMetadata(_init, _Foo);
-(__publicField(_Foo, "foo", __runInitializers(_init, 8, _Foo, _Foo)), __runInitializers(_init, 11, _Foo));
-var Foo = _Foo;
+//#region base-static-field.js
+var Foo = class Foo {
+	@dec static foo = Foo;
+};
+
+//#endregion
\ No newline at end of file

```
## /out/base-static-method.js
### esbuild
```js
// base-static-method.js
var _foo_dec, _init;
_foo_dec = [dec];
var _Foo = class _Foo {
  static foo() {
    return _Foo;
  }
};
_init = __decoratorStart(null);
__decorateElement(_init, 9, "foo", _foo_dec, _Foo);
__decoratorMetadata(_init, _Foo);
__runInitializers(_init, 3, _Foo);
var Foo = _Foo;
```
### rolldown
```js
//#region base-static-method.js
var Foo = class Foo {
	@dec static foo() {
		return Foo;
	}
};

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/base-static-method.js
+++ rolldown	base-static-method.js
@@ -1,12 +1,8 @@
-var _foo_dec, _init;
-_foo_dec = [dec];
-var _Foo = class _Foo {
-    static foo() {
-        return _Foo;
-    }
+//#region base-static-method.js
+var Foo = class Foo {
+	@dec static foo() {
+		return Foo;
+	}
 };
-_init = __decoratorStart(null);
-__decorateElement(_init, 9, "foo", _foo_dec, _Foo);
-__decoratorMetadata(_init, _Foo);
-__runInitializers(_init, 3, _Foo);
-var Foo = _Foo;
+
+//#endregion
\ No newline at end of file

```
## /out/derived-instance-accessor.js
### esbuild
```js
// derived-instance-accessor.js
var _foo_dec, _a, _init, _foo;
var _Foo = class _Foo extends (_a = Bar, _foo_dec = [dec], _a) {
  constructor() {
    super(...arguments);
    __privateAdd(this, _foo, __runInitializers(_init, 8, this, _Foo)), __runInitializers(_init, 11, this);
  }
};
_init = __decoratorStart(_a);
_foo = new WeakMap();
__decorateElement(_init, 4, "foo", _foo_dec, _Foo, _foo);
__decoratorMetadata(_init, _Foo);
var Foo = _Foo;
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/derived-instance-accessor.js
+++ rolldown	derived-instance-accessor.js
@@ -1,12 +0,0 @@
-var _foo_dec, _a, _init, _foo;
-var _Foo = class _Foo extends (_a = Bar, _foo_dec = [dec], _a) {
-    constructor() {
-        super(...arguments);
-        (__privateAdd(this, _foo, __runInitializers(_init, 8, this, _Foo)), __runInitializers(_init, 11, this));
-    }
-};
-_init = __decoratorStart(_a);
-_foo = new WeakMap();
-__decorateElement(_init, 4, "foo", _foo_dec, _Foo, _foo);
-__decoratorMetadata(_init, _Foo);
-var Foo = _Foo;

```
## /out/derived-instance-field.js
### esbuild
```js
// derived-instance-field.js
var _foo_dec, _a, _init;
var _Foo = class _Foo extends (_a = Bar, _foo_dec = [dec], _a) {
  constructor() {
    super(...arguments);
    __publicField(this, "foo", __runInitializers(_init, 8, this, _Foo)), __runInitializers(_init, 11, this);
  }
};
_init = __decoratorStart(_a);
__decorateElement(_init, 5, "foo", _foo_dec, _Foo);
__decoratorMetadata(_init, _Foo);
var Foo = _Foo;
```
### rolldown
```js
//#region derived-instance-field.js
var Foo = class Foo extends Bar {
	@dec foo = Foo;
};

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/derived-instance-field.js
+++ rolldown	derived-instance-field.js
@@ -1,11 +1,6 @@
-var _foo_dec, _a, _init;
-var _Foo = class _Foo extends (_a = Bar, _foo_dec = [dec], _a) {
-    constructor() {
-        super(...arguments);
-        (__publicField(this, "foo", __runInitializers(_init, 8, this, _Foo)), __runInitializers(_init, 11, this));
-    }
+//#region derived-instance-field.js
+var Foo = class Foo extends Bar {
+	@dec foo = Foo;
 };
-_init = __decoratorStart(_a);
-__decorateElement(_init, 5, "foo", _foo_dec, _Foo);
-__decoratorMetadata(_init, _Foo);
-var Foo = _Foo;
+
+//#endregion
\ No newline at end of file

```
## /out/derived-instance-method.js
### esbuild
```js
// derived-instance-method.js
var _foo_dec, _a, _init;
var _Foo = class _Foo extends (_a = Bar, _foo_dec = [dec], _a) {
  constructor() {
    super(...arguments);
    __runInitializers(_init, 5, this);
  }
  foo() {
    return _Foo;
  }
};
_init = __decoratorStart(_a);
__decorateElement(_init, 1, "foo", _foo_dec, _Foo);
__decoratorMetadata(_init, _Foo);
var Foo = _Foo;
```
### rolldown
```js
//#region derived-instance-method.js
var Foo = class Foo extends Bar {
	@dec foo() {
		return Foo;
	}
};

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/derived-instance-method.js
+++ rolldown	derived-instance-method.js
@@ -1,14 +1,8 @@
-var _foo_dec, _a, _init;
-var _Foo = class _Foo extends (_a = Bar, _foo_dec = [dec], _a) {
-    constructor() {
-        super(...arguments);
-        __runInitializers(_init, 5, this);
-    }
-    foo() {
-        return _Foo;
-    }
+//#region derived-instance-method.js
+var Foo = class Foo extends Bar {
+	@dec foo() {
+		return Foo;
+	}
 };
-_init = __decoratorStart(_a);
-__decorateElement(_init, 1, "foo", _foo_dec, _Foo);
-__decoratorMetadata(_init, _Foo);
-var Foo = _Foo;
+
+//#endregion
\ No newline at end of file

```
## /out/derived-static-accessor.js
### esbuild
```js
// derived-static-accessor.js
var _foo_dec, _a, _init, _foo;
var _Foo = class _Foo extends (_a = Bar, _foo_dec = [dec], _a) {
};
_init = __decoratorStart(_a);
_foo = new WeakMap();
__decorateElement(_init, 12, "foo", _foo_dec, _Foo, _foo);
__decoratorMetadata(_init, _Foo);
__privateAdd(_Foo, _foo, __runInitializers(_init, 8, _Foo, _Foo)), __runInitializers(_init, 11, _Foo);
var Foo = _Foo;
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/derived-static-accessor.js
+++ rolldown	derived-static-accessor.js
@@ -1,8 +0,0 @@
-var _foo_dec, _a, _init, _foo;
-var _Foo = class _Foo extends (_a = Bar, _foo_dec = [dec], _a) {};
-_init = __decoratorStart(_a);
-_foo = new WeakMap();
-__decorateElement(_init, 12, "foo", _foo_dec, _Foo, _foo);
-__decoratorMetadata(_init, _Foo);
-(__privateAdd(_Foo, _foo, __runInitializers(_init, 8, _Foo, _Foo)), __runInitializers(_init, 11, _Foo));
-var Foo = _Foo;

```
## /out/derived-static-field.js
### esbuild
```js
// derived-static-field.js
var _foo_dec, _a, _init;
var _Foo = class _Foo extends (_a = Bar, _foo_dec = [dec], _a) {
};
_init = __decoratorStart(_a);
__decorateElement(_init, 13, "foo", _foo_dec, _Foo);
__decoratorMetadata(_init, _Foo);
__publicField(_Foo, "foo", __runInitializers(_init, 8, _Foo, _Foo)), __runInitializers(_init, 11, _Foo);
var Foo = _Foo;
```
### rolldown
```js
//#region derived-static-field.js
var Foo = class Foo extends Bar {
	@dec static foo = Foo;
};

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/derived-static-field.js
+++ rolldown	derived-static-field.js
@@ -1,7 +1,6 @@
-var _foo_dec, _a, _init;
-var _Foo = class _Foo extends (_a = Bar, _foo_dec = [dec], _a) {};
-_init = __decoratorStart(_a);
-__decorateElement(_init, 13, "foo", _foo_dec, _Foo);
-__decoratorMetadata(_init, _Foo);
-(__publicField(_Foo, "foo", __runInitializers(_init, 8, _Foo, _Foo)), __runInitializers(_init, 11, _Foo));
-var Foo = _Foo;
+//#region derived-static-field.js
+var Foo = class Foo extends Bar {
+	@dec static foo = Foo;
+};
+
+//#endregion
\ No newline at end of file

```
## /out/derived-static-method.js
### esbuild
```js
// derived-static-method.js
var _foo_dec, _a, _init;
var _Foo = class _Foo extends (_a = Bar, _foo_dec = [dec], _a) {
  static foo() {
    return _Foo;
  }
};
_init = __decoratorStart(_a);
__decorateElement(_init, 9, "foo", _foo_dec, _Foo);
__decoratorMetadata(_init, _Foo);
__runInitializers(_init, 3, _Foo);
var Foo = _Foo;
```
### rolldown
```js
//#region derived-static-method.js
var Foo = class Foo extends Bar {
	@dec static foo() {
		return Foo;
	}
};

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/derived-static-method.js
+++ rolldown	derived-static-method.js
@@ -1,11 +1,8 @@
-var _foo_dec, _a, _init;
-var _Foo = class _Foo extends (_a = Bar, _foo_dec = [dec], _a) {
-    static foo() {
-        return _Foo;
-    }
+//#region derived-static-method.js
+var Foo = class Foo extends Bar {
+	@dec static foo() {
+		return Foo;
+	}
 };
-_init = __decoratorStart(_a);
-__decorateElement(_init, 9, "foo", _foo_dec, _Foo);
-__decoratorMetadata(_init, _Foo);
-__runInitializers(_init, 3, _Foo);
-var Foo = _Foo;
+
+//#endregion
\ No newline at end of file

```