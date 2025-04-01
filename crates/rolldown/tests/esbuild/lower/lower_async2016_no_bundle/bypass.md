# Reason
1. pure transformation is handled by `oxc-transform`
# Diff
## /out.js
### esbuild
```js
function foo(_0) {
  return __async(this, arguments, function* (bar) {
    yield bar;
    return [this, arguments];
  });
}
class Foo {
  foo() {
    return __async(this, null, function* () {
    });
  }
}
export default [
  foo,
  Foo,
  function() {
    return __async(this, null, function* () {
    });
  },
  () => __async(void 0, null, function* () {
  }),
  { foo() {
    return __async(this, null, function* () {
    });
  } },
  class {
    foo() {
      return __async(this, null, function* () {
      });
    }
  },
  function() {
    var _arguments = arguments;
    return (bar) => __async(this, null, function* () {
      yield bar;
      return [this, _arguments];
    });
  }
];
```
### rolldown
```js

//#region entry.js
async function foo(bar) {
	await bar;
	return [this, arguments];
}
var Foo = class {
	async foo() {}
};
var entry_default = [
	foo,
	Foo,
	async function() {},
	async () => {},
	{ async foo() {} },
	class {
		async foo() {}
	},
	function() {
		return async (bar) => {
			await bar;
			return [this, arguments];
		};
	}
];
//#endregion

export { entry_default as default };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,28 +1,18 @@
-function foo(_0) {
-    return __async(this, arguments, function* (bar) {
-        yield bar;
-        return [this, arguments];
-    });
+async function foo(bar) {
+    await bar;
+    return [this, arguments];
 }
-class Foo {
-    foo() {
-        return __async(this, null, function* () {});
-    }
-}
-export default [foo, Foo, function () {
-    return __async(this, null, function* () {});
-}, () => __async(void 0, null, function* () {}), {
-    foo() {
-        return __async(this, null, function* () {});
-    }
+var Foo = class {
+    async foo() {}
+};
+var entry_default = [foo, Foo, async function () {}, async () => {}, {
+    async foo() {}
 }, class {
-    foo() {
-        return __async(this, null, function* () {});
-    }
+    async foo() {}
 }, function () {
-    var _arguments = arguments;
-    return bar => __async(this, null, function* () {
-        yield bar;
-        return [this, _arguments];
-    });
+    return async bar => {
+        await bar;
+        return [this, arguments];
+    };
 }];
+export {entry_default as default};

```