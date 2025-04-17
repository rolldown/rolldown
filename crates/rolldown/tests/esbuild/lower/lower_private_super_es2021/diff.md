# Diff
## /out.js
### esbuild
```js
// foo1.js
var _default_instances, foo_fn;
var _foo1_default = class _foo1_default extends x {
  constructor() {
    super(...arguments);
    __privateAdd(this, _default_instances);
  }
};
_default_instances = new WeakSet();
foo_fn = function() {
  __superGet(_foo1_default.prototype, this, "foo").call(this);
};
var foo1_default = _foo1_default;

// foo2.js
var _default_instances2, foo_fn2;
var _foo2_default = class _foo2_default extends x {
  constructor() {
    super(...arguments);
    __privateAdd(this, _default_instances2);
  }
};
_default_instances2 = new WeakSet();
foo_fn2 = function() {
  __superWrapper(_foo2_default.prototype, this, "foo")._++;
};
var foo2_default = _foo2_default;

// foo3.js
var _default_static, foo_fn3;
var _foo3_default = class _foo3_default extends x {
};
_default_static = new WeakSet();
foo_fn3 = function() {
  __superGet(_foo3_default, this, "foo").call(this);
};
__privateAdd(_foo3_default, _default_static);
var foo3_default = _foo3_default;

// foo4.js
var _default_static2, foo_fn4;
var _foo4_default = class _foo4_default extends x {
};
_default_static2 = new WeakSet();
foo_fn4 = function() {
  __superWrapper(_foo4_default, this, "foo")._++;
};
__privateAdd(_foo4_default, _default_static2);
var foo4_default = _foo4_default;

// foo5.js
var _foo;
var foo5_default = class extends x {
  constructor() {
    super(...arguments);
    __privateAdd(this, _foo, () => {
      super.foo();
    });
  }
};
_foo = new WeakMap();

// foo6.js
var _foo2;
var foo6_default = class extends x {
  constructor() {
    super(...arguments);
    __privateAdd(this, _foo2, () => {
      super.foo++;
    });
  }
};
_foo2 = new WeakMap();

// foo7.js
var _foo3;
var _foo7_default = class _foo7_default extends x {
};
_foo3 = new WeakMap();
__privateAdd(_foo7_default, _foo3, () => {
  __superGet(_foo7_default, _foo7_default, "foo").call(this);
});
var foo7_default = _foo7_default;

// foo8.js
var _foo4;
var _foo8_default = class _foo8_default extends x {
};
_foo4 = new WeakMap();
__privateAdd(_foo8_default, _foo4, () => {
  __superWrapper(_foo8_default, _foo8_default, "foo")._++;
});
var foo8_default = _foo8_default;
export {
  foo1_default as foo1,
  foo2_default as foo2,
  foo3_default as foo3,
  foo4_default as foo4,
  foo5_default as foo5,
  foo6_default as foo6,
  foo7_default as foo7,
  foo8_default as foo8
};
```
### rolldown
```js

//#region foo1.js
var foo1_default = class extends x {
	#foo() {
		super.foo();
	}
};

//#region foo2.js
var foo2_default = class extends x {
	#foo() {
		super.foo++;
	}
};

//#region foo3.js
var foo3_default = class extends x {
	static #foo() {
		super.foo();
	}
};

//#region foo4.js
var foo4_default = class extends x {
	static #foo() {
		super.foo++;
	}
};

//#region foo5.js
var foo5_default = class extends x {
	#foo = () => {
		super.foo();
	};
};

//#region foo6.js
var foo6_default = class extends x {
	#foo = () => {
		super.foo++;
	};
};

//#region foo7.js
var foo7_default = class extends x {
	static #foo = () => {
		super.foo();
	};
};

//#region foo8.js
var foo8_default = class extends x {
	static #foo = () => {
		super.foo++;
	};
};

export { foo1_default as foo1, foo2_default as foo2, foo3_default as foo3, foo4_default as foo4, foo5_default as foo5, foo6_default as foo6, foo7_default as foo7, foo8_default as foo8 };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,75 +1,41 @@
-var _default_instances, foo_fn;
-var _foo1_default = class _foo1_default extends x {
-    constructor() {
-        super(...arguments);
-        __privateAdd(this, _default_instances);
+var foo1_default = class extends x {
+    #foo() {
+        super.foo();
     }
 };
-_default_instances = new WeakSet();
-foo_fn = function () {
-    __superGet(_foo1_default.prototype, this, "foo").call(this);
+var foo2_default = class extends x {
+    #foo() {
+        super.foo++;
+    }
 };
-var foo1_default = _foo1_default;
-var _default_instances2, foo_fn2;
-var _foo2_default = class _foo2_default extends x {
-    constructor() {
-        super(...arguments);
-        __privateAdd(this, _default_instances2);
+var foo3_default = class extends x {
+    static #foo() {
+        super.foo();
     }
 };
-_default_instances2 = new WeakSet();
-foo_fn2 = function () {
-    __superWrapper(_foo2_default.prototype, this, "foo")._++;
+var foo4_default = class extends x {
+    static #foo() {
+        super.foo++;
+    }
 };
-var foo2_default = _foo2_default;
-var _default_static, foo_fn3;
-var _foo3_default = class _foo3_default extends x {};
-_default_static = new WeakSet();
-foo_fn3 = function () {
-    __superGet(_foo3_default, this, "foo").call(this);
-};
-__privateAdd(_foo3_default, _default_static);
-var foo3_default = _foo3_default;
-var _default_static2, foo_fn4;
-var _foo4_default = class _foo4_default extends x {};
-_default_static2 = new WeakSet();
-foo_fn4 = function () {
-    __superWrapper(_foo4_default, this, "foo")._++;
-};
-__privateAdd(_foo4_default, _default_static2);
-var foo4_default = _foo4_default;
-var _foo;
 var foo5_default = class extends x {
-    constructor() {
-        super(...arguments);
-        __privateAdd(this, _foo, () => {
-            super.foo();
-        });
-    }
+    #foo = () => {
+        super.foo();
+    };
 };
-_foo = new WeakMap();
-var _foo2;
 var foo6_default = class extends x {
-    constructor() {
-        super(...arguments);
-        __privateAdd(this, _foo2, () => {
-            super.foo++;
-        });
-    }
+    #foo = () => {
+        super.foo++;
+    };
 };
-_foo2 = new WeakMap();
-var _foo3;
-var _foo7_default = class _foo7_default extends x {};
-_foo3 = new WeakMap();
-__privateAdd(_foo7_default, _foo3, () => {
-    __superGet(_foo7_default, _foo7_default, "foo").call(this);
-});
-var foo7_default = _foo7_default;
-var _foo4;
-var _foo8_default = class _foo8_default extends x {};
-_foo4 = new WeakMap();
-__privateAdd(_foo8_default, _foo4, () => {
-    __superWrapper(_foo8_default, _foo8_default, "foo")._++;
-});
-var foo8_default = _foo8_default;
+var foo7_default = class extends x {
+    static #foo = () => {
+        super.foo();
+    };
+};
+var foo8_default = class extends x {
+    static #foo = () => {
+        super.foo++;
+    };
+};
 export {foo1_default as foo1, foo2_default as foo2, foo3_default as foo3, foo4_default as foo4, foo5_default as foo5, foo6_default as foo6, foo7_default as foo7, foo8_default as foo8};

```