# Diff
## /out.js
### esbuild
```js
// entry.js
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
export {
  Foo
};
```
### rolldown
```js

//#region entry.js
class Foo {
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
     get #foo() {
         return this.foo;
     }
     set #bar(val) {
@@ -40,6 +40,6 @@
         fn().#prop &&= 1;
         fn().#prop ||= 1;
         fn().#prop ??= 1;
     }
-};
+}
 export {Foo};

```