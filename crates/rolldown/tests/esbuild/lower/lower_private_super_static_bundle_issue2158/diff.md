# Diff
## /out.js
### esbuild
```js
// entry.js
var Foo = class extends Object {
  static FOO;
  constructor() {
    super();
  }
  #foo;
};
export {
  Foo
};
```
### rolldown
```js

//#region entry.js
class Foo extends Object {
	static FOO;
	constructor() {
		super();
	}
	#foo;
}

//#endregion
export { Foo };

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,8 @@
-var Foo = class extends Object {
+class Foo extends Object {
     static FOO;
     constructor() {
         super();
     }
     #foo;
-};
+}
 export {Foo};

```