# Reason
1. pure transformation is handled by `oxc-transform`
# Diff
## /out.js
### esbuild
```js
async function foo(bar) {
  await bar;
  return arguments;
}
class Foo {
  async foo() {
  }
}
export default [
  foo,
  Foo,
  async function() {
  },
  async () => {
  },
  { async foo() {
  } },
  class {
    async foo() {
    }
  },
  function() {
    return async (bar) => {
      await bar;
      return [this, arguments];
    };
  }
];
```
### rolldown
```js
//#region entry.js
async function foo(bar) {
	await bar;
	return arguments;
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
@@ -1,12 +1,12 @@
 async function foo(bar) {
     await bar;
     return arguments;
 }
-class Foo {
+var Foo = class {
     async foo() {}
-}
-export default [foo, Foo, async function () {}, async () => {}, {
+};
+var entry_default = [foo, Foo, async function () {}, async () => {}, {
     async foo() {}
 }, class {
     async foo() {}
 }, function () {
@@ -14,4 +14,5 @@
         await bar;
         return [this, arguments];
     };
 }];
+export {entry_default as default};

```