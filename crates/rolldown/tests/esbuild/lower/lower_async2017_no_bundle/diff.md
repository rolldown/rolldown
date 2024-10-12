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
class Foo {
	async foo() {}
}
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
@@ -4,9 +4,9 @@
 }
 class Foo {
     async foo() {}
 }
-export default [foo, Foo, async function () {}, async () => {}, {
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