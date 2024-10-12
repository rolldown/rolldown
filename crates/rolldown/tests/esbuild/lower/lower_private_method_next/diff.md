# Diff
## /out.js
### esbuild
```js
// entry.js
var Foo = class {
  #field;
  #method() {
  }
  baseline() {
    a().foo;
    b().foo(x);
    c()?.foo(x);
    d().foo?.(x);
    e()?.foo?.(x);
  }
  privateField() {
    a().#field;
    b().#field(x);
    c()?.#field(x);
    d().#field?.(x);
    e()?.#field?.(x);
    f()?.foo.#field(x).bar();
  }
  privateMethod() {
    a().#method;
    b().#method(x);
    c()?.#method(x);
    d().#method?.(x);
    e()?.#method?.(x);
    f()?.foo.#method(x).bar();
  }
};
export {
  Foo
};
```
### rolldown
```js

//#region entry.js
class Foo {
	#field;
	#method() {}
	baseline() {
		a().foo;
		b().foo(x);
		c()?.foo(x);
		d().foo?.(x);
		e()?.foo?.(x);
	}
	privateField() {
		a().#field;
		b().#field(x);
		c()?.#field(x);
		d().#field?.(x);
		e()?.#field?.(x);
		f()?.foo.#field(x).bar();
	}
	privateMethod() {
		a().#method;
		b().#method(x);
		c()?.#method(x);
		d().#method?.(x);
		e()?.#method?.(x);
		f()?.foo.#method(x).bar();
	}
}

//#endregion
export { Foo };

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,5 +1,5 @@
-var Foo = class {
+class Foo {
     #field;
     #method() {}
     baseline() {
         a().foo;
@@ -23,6 +23,6 @@
         d().#method?.(x);
         e()?.#method?.(x);
         f()?.foo.#method(x).bar();
     }
-};
+}
 export {Foo};

```