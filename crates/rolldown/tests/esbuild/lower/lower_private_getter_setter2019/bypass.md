# Reason
1. pure transformation is handled by `oxc-transform`
# Diff
## /out.js
### esbuild
```js
// entry.js
var _Foo_instances, foo_get, bar_set, prop_get, prop_set;
var Foo = class {
  constructor() {
    __privateAdd(this, _Foo_instances);
  }
  foo(fn) {
    __privateGet(fn(), _Foo_instances, foo_get);
    __privateSet(fn(), _Foo_instances, 1, bar_set);
    __privateGet(fn(), _Foo_instances, prop_get);
    __privateSet(fn(), _Foo_instances, 2, prop_set);
  }
  unary(fn) {
    __privateWrapper(fn(), _Foo_instances, prop_set, prop_get)._++;
    __privateWrapper(fn(), _Foo_instances, prop_set, prop_get)._--;
    ++__privateWrapper(fn(), _Foo_instances, prop_set, prop_get)._;
    --__privateWrapper(fn(), _Foo_instances, prop_set, prop_get)._;
  }
  binary(fn) {
    var _a, _b, _c, _d, _e, _f, _g, _h, _i, _j, _k, _l, _m, _n, _o, _p;
    __privateSet(fn(), _Foo_instances, 1, prop_set);
    __privateSet(_a = fn(), _Foo_instances, __privateGet(_a, _Foo_instances, prop_get) + 1, prop_set);
    __privateSet(_b = fn(), _Foo_instances, __privateGet(_b, _Foo_instances, prop_get) - 1, prop_set);
    __privateSet(_c = fn(), _Foo_instances, __privateGet(_c, _Foo_instances, prop_get) * 1, prop_set);
    __privateSet(_d = fn(), _Foo_instances, __privateGet(_d, _Foo_instances, prop_get) / 1, prop_set);
    __privateSet(_e = fn(), _Foo_instances, __privateGet(_e, _Foo_instances, prop_get) % 1, prop_set);
    __privateSet(_f = fn(), _Foo_instances, __privateGet(_f, _Foo_instances, prop_get) ** 1, prop_set);
    __privateSet(_g = fn(), _Foo_instances, __privateGet(_g, _Foo_instances, prop_get) << 1, prop_set);
    __privateSet(_h = fn(), _Foo_instances, __privateGet(_h, _Foo_instances, prop_get) >> 1, prop_set);
    __privateSet(_i = fn(), _Foo_instances, __privateGet(_i, _Foo_instances, prop_get) >>> 1, prop_set);
    __privateSet(_j = fn(), _Foo_instances, __privateGet(_j, _Foo_instances, prop_get) & 1, prop_set);
    __privateSet(_k = fn(), _Foo_instances, __privateGet(_k, _Foo_instances, prop_get) | 1, prop_set);
    __privateSet(_l = fn(), _Foo_instances, __privateGet(_l, _Foo_instances, prop_get) ^ 1, prop_set);
    __privateGet(_m = fn(), _Foo_instances, prop_get) && __privateSet(_m, _Foo_instances, 1, prop_set);
    __privateGet(_n = fn(), _Foo_instances, prop_get) || __privateSet(_n, _Foo_instances, 1, prop_set);
    (_p = __privateGet(_o = fn(), _Foo_instances, prop_get)) != null ? _p : __privateSet(_o, _Foo_instances, 1, prop_set);
  }
};
_Foo_instances = new WeakSet();
foo_get = function() {
  return this.foo;
};
bar_set = function(val) {
  this.bar = val;
};
prop_get = function() {
  return this.prop;
};
prop_set = function(val) {
  this.prop = val;
};
export {
  Foo
};
```
### rolldown
```js
//#region entry.js
var Foo = class {
	get #foo() {
		return this.foo;
	}
	set #bar(val) {
		this.bar = val;
	}
	get #prop() {
		return this.prop;
	}
	set #prop(val) {
		this.prop = val;
	}
	foo(fn) {
		fn().#foo;
		fn().#bar = 1;
		fn().#prop;
		fn().#prop = 2;
	}
	unary(fn) {
		fn().#prop++;
		fn().#prop--;
		++fn().#prop;
		--fn().#prop;
	}
	binary(fn) {
		fn().#prop = 1;
		fn().#prop += 1;
		fn().#prop -= 1;
		fn().#prop *= 1;
		fn().#prop /= 1;
		fn().#prop %= 1;
		fn().#prop **= 1;
		fn().#prop <<= 1;
		fn().#prop >>= 1;
		fn().#prop >>>= 1;
		fn().#prop &= 1;
		fn().#prop |= 1;
		fn().#prop ^= 1;
		fn().#prop &&= 1;
		fn().#prop ||= 1;
		fn().#prop ??= 1;
	}
};

//#endregion
export { Foo };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,51 +1,45 @@
-var _Foo_instances, foo_get, bar_set, prop_get, prop_set;
 var Foo = class {
-    constructor() {
-        __privateAdd(this, _Foo_instances);
+    get #foo() {
+        return this.foo;
     }
+    set #bar(val) {
+        this.bar = val;
+    }
+    get #prop() {
+        return this.prop;
+    }
+    set #prop(val) {
+        this.prop = val;
+    }
     foo(fn) {
-        __privateGet(fn(), _Foo_instances, foo_get);
-        __privateSet(fn(), _Foo_instances, 1, bar_set);
-        __privateGet(fn(), _Foo_instances, prop_get);
-        __privateSet(fn(), _Foo_instances, 2, prop_set);
+        fn().#foo;
+        fn().#bar = 1;
+        fn().#prop;
+        fn().#prop = 2;
     }
     unary(fn) {
-        __privateWrapper(fn(), _Foo_instances, prop_set, prop_get)._++;
-        __privateWrapper(fn(), _Foo_instances, prop_set, prop_get)._--;
-        ++__privateWrapper(fn(), _Foo_instances, prop_set, prop_get)._;
-        --__privateWrapper(fn(), _Foo_instances, prop_set, prop_get)._;
+        fn().#prop++;
+        fn().#prop--;
+        ++fn().#prop;
+        --fn().#prop;
     }
     binary(fn) {
-        var _a, _b, _c, _d, _e, _f, _g, _h, _i, _j, _k, _l, _m, _n, _o, _p;
-        __privateSet(fn(), _Foo_instances, 1, prop_set);
-        __privateSet(_a = fn(), _Foo_instances, __privateGet(_a, _Foo_instances, prop_get) + 1, prop_set);
-        __privateSet(_b = fn(), _Foo_instances, __privateGet(_b, _Foo_instances, prop_get) - 1, prop_set);
-        __privateSet(_c = fn(), _Foo_instances, __privateGet(_c, _Foo_instances, prop_get) * 1, prop_set);
-        __privateSet(_d = fn(), _Foo_instances, __privateGet(_d, _Foo_instances, prop_get) / 1, prop_set);
-        __privateSet(_e = fn(), _Foo_instances, __privateGet(_e, _Foo_instances, prop_get) % 1, prop_set);
-        __privateSet(_f = fn(), _Foo_instances, __privateGet(_f, _Foo_instances, prop_get) ** 1, prop_set);
-        __privateSet(_g = fn(), _Foo_instances, __privateGet(_g, _Foo_instances, prop_get) << 1, prop_set);
-        __privateSet(_h = fn(), _Foo_instances, __privateGet(_h, _Foo_instances, prop_get) >> 1, prop_set);
-        __privateSet(_i = fn(), _Foo_instances, __privateGet(_i, _Foo_instances, prop_get) >>> 1, prop_set);
-        __privateSet(_j = fn(), _Foo_instances, __privateGet(_j, _Foo_instances, prop_get) & 1, prop_set);
-        __privateSet(_k = fn(), _Foo_instances, __privateGet(_k, _Foo_instances, prop_get) | 1, prop_set);
-        __privateSet(_l = fn(), _Foo_instances, __privateGet(_l, _Foo_instances, prop_get) ^ 1, prop_set);
-        __privateGet(_m = fn(), _Foo_instances, prop_get) && __privateSet(_m, _Foo_instances, 1, prop_set);
-        __privateGet(_n = fn(), _Foo_instances, prop_get) || __privateSet(_n, _Foo_instances, 1, prop_set);
-        (_p = __privateGet(_o = fn(), _Foo_instances, prop_get)) != null ? _p : __privateSet(_o, _Foo_instances, 1, prop_set);
+        fn().#prop = 1;
+        fn().#prop += 1;
+        fn().#prop -= 1;
+        fn().#prop *= 1;
+        fn().#prop /= 1;
+        fn().#prop %= 1;
+        fn().#prop **= 1;
+        fn().#prop <<= 1;
+        fn().#prop >>= 1;
+        fn().#prop >>>= 1;
+        fn().#prop &= 1;
+        fn().#prop |= 1;
+        fn().#prop ^= 1;
+        fn().#prop &&= 1;
+        fn().#prop ||= 1;
+        fn().#prop ??= 1;
     }
 };
-_Foo_instances = new WeakSet();
-foo_get = function () {
-    return this.foo;
-};
-bar_set = function (val) {
-    this.bar = val;
-};
-prop_get = function () {
-    return this.prop;
-};
-prop_set = function (val) {
-    this.prop = val;
-};
 export {Foo};

```