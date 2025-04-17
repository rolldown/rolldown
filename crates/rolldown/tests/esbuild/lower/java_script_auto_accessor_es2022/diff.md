# Diff
## /out/js-define.js
### esbuild
```js
var _a, _b;
class Foo {
  #one = 1;
  get one() {
    return this.#one;
  }
  set one(_) {
    this.#one = _;
  }
  #_two = 2;
  get #two() {
    return this.#_two;
  }
  set #two(_) {
    this.#_two = _;
  }
  #a = 3;
  get [_b = three()]() {
    return this.#a;
  }
  set [_b](_) {
    this.#a = _;
  }
  static #four = 4;
  static get four() {
    return this.#four;
  }
  static set four(_) {
    this.#four = _;
  }
  static #_five = 5;
  static get #five() {
    return this.#_five;
  }
  static set #five(_) {
    this.#_five = _;
  }
  static #b = 6;
  static get [_a = six()]() {
    return this.#b;
  }
  static set [_a](_) {
    this.#b = _;
  }
}
```
### rolldown
```js

//#region js-define.js
var Foo = class {
	accessor one = 1;
	accessor #two = 2;
	accessor [three()] = 3;
	static accessor four = 4;
	static accessor #five = 5;
	static accessor [six()] = 6;
};

```
### diff
```diff
===================================================================
--- esbuild	/out/js-define.js
+++ rolldown	js-define.js
@@ -1,45 +1,10 @@
-var _a, _b;
-class Foo {
-    #one = 1;
-    get one() {
-        return this.#one;
-    }
-    set one(_) {
-        this.#one = _;
-    }
-    #_two = 2;
-    get #two() {
-        return this.#_two;
-    }
-    set #two(_) {
-        this.#_two = _;
-    }
-    #a = 3;
-    get [_b = three()]() {
-        return this.#a;
-    }
-    set [_b](_) {
-        this.#a = _;
-    }
-    static #four = 4;
-    static get four() {
-        return this.#four;
-    }
-    static set four(_) {
-        this.#four = _;
-    }
-    static #_five = 5;
-    static get #five() {
-        return this.#_five;
-    }
-    static set #five(_) {
-        this.#_five = _;
-    }
-    static #b = 6;
-    static get [_a = six()]() {
-        return this.#b;
-    }
-    static set [_a](_) {
-        this.#b = _;
-    }
-}
+
+//#region js-define.js
+var Foo = class {
+	accessor one = 1;
+	accessor #two = 2;
+	accessor [three()] = 3;
+	static accessor four = 4;
+	static accessor #five = 5;
+	static accessor [six()] = 6;
+};

```
## /out/ts-define/ts-define.js
### esbuild
```js
var _a, _b;
class Foo {
  #one = 1;
  get one() {
    return this.#one;
  }
  set one(_) {
    this.#one = _;
  }
  #_two = 2;
  get #two() {
    return this.#_two;
  }
  set #two(_) {
    this.#_two = _;
  }
  #a = 3;
  get [_b = three()]() {
    return this.#a;
  }
  set [_b](_) {
    this.#a = _;
  }
  static #four = 4;
  static get four() {
    return this.#four;
  }
  static set four(_) {
    this.#four = _;
  }
  static #_five = 5;
  static get #five() {
    return this.#_five;
  }
  static set #five(_) {
    this.#_five = _;
  }
  static #b = 6;
  static get [_a = six()]() {
    return this.#b;
  }
  static set [_a](_) {
    this.#b = _;
  }
}
class Normal {
  #a = b;
  get a() {
    return this.#a;
  }
  set a(_) {
    this.#a = _;
  }
  c = d;
}
class Private {
  #_a = b;
  get #a() {
    return this.#_a;
  }
  set #a(_) {
    this.#_a = _;
  }
  c = d;
}
class StaticNormal {
  static #a = b;
  static get a() {
    return this.#a;
  }
  static set a(_) {
    this.#a = _;
  }
  static c = d;
}
class StaticPrivate {
  static #_a = b;
  static get #a() {
    return this.#_a;
  }
  static set #a(_) {
    this.#_a = _;
  }
  static c = d;
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/ts-define/ts-define.js
+++ rolldown	
@@ -1,85 +0,0 @@
-var _a, _b;
-class Foo {
-    #one = 1;
-    get one() {
-        return this.#one;
-    }
-    set one(_) {
-        this.#one = _;
-    }
-    #_two = 2;
-    get #two() {
-        return this.#_two;
-    }
-    set #two(_) {
-        this.#_two = _;
-    }
-    #a = 3;
-    get [_b = three()]() {
-        return this.#a;
-    }
-    set [_b](_) {
-        this.#a = _;
-    }
-    static #four = 4;
-    static get four() {
-        return this.#four;
-    }
-    static set four(_) {
-        this.#four = _;
-    }
-    static #_five = 5;
-    static get #five() {
-        return this.#_five;
-    }
-    static set #five(_) {
-        this.#_five = _;
-    }
-    static #b = 6;
-    static get [_a = six()]() {
-        return this.#b;
-    }
-    static set [_a](_) {
-        this.#b = _;
-    }
-}
-class Normal {
-    #a = b;
-    get a() {
-        return this.#a;
-    }
-    set a(_) {
-        this.#a = _;
-    }
-    c = d;
-}
-class Private {
-    #_a = b;
-    get #a() {
-        return this.#_a;
-    }
-    set #a(_) {
-        this.#_a = _;
-    }
-    c = d;
-}
-class StaticNormal {
-    static #a = b;
-    static get a() {
-        return this.#a;
-    }
-    static set a(_) {
-        this.#a = _;
-    }
-    static c = d;
-}
-class StaticPrivate {
-    static #_a = b;
-    static get #a() {
-        return this.#_a;
-    }
-    static set #a(_) {
-        this.#_a = _;
-    }
-    static c = d;
-}

```
## /out/ts-assign/ts-assign.js
### esbuild
```js
var _a, _b, _a2, __a;
class Foo {
  #one = 1;
  get one() {
    return this.#one;
  }
  set one(_) {
    this.#one = _;
  }
  #_two = 2;
  get #two() {
    return this.#_two;
  }
  set #two(_) {
    this.#_two = _;
  }
  #a = 3;
  get [_b = three()]() {
    return this.#a;
  }
  set [_b](_) {
    this.#a = _;
  }
  static #four = 4;
  static get four() {
    return this.#four;
  }
  static set four(_) {
    this.#four = _;
  }
  static #_five = 5;
  static get #five() {
    return this.#_five;
  }
  static set #five(_) {
    this.#_five = _;
  }
  static #b = 6;
  static get [_a = six()]() {
    return this.#b;
  }
  static set [_a](_) {
    this.#b = _;
  }
}
class Normal {
  constructor() {
    __privateAdd(this, _a2, b);
    this.c = d;
  }
  get a() {
    return __privateGet(this, _a2);
  }
  set a(_) {
    __privateSet(this, _a2, _);
  }
}
_a2 = new WeakMap();
class Private {
  constructor() {
    __privateAdd(this, __a, b);
    this.c = d;
  }
  get #a() {
    return __privateGet(this, __a);
  }
  set #a(_) {
    __privateSet(this, __a, _);
  }
}
__a = new WeakMap();
class StaticNormal {
  static #a = b;
  static get a() {
    return this.#a;
  }
  static set a(_) {
    this.#a = _;
  }
  static {
    this.c = d;
  }
}
class StaticPrivate {
  static #_a = b;
  static get #a() {
    return this.#_a;
  }
  static set #a(_) {
    this.#_a = _;
  }
  static {
    this.c = d;
  }
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/ts-assign/ts-assign.js
+++ rolldown	
@@ -1,95 +0,0 @@
-var _a, _b, _a2, __a;
-class Foo {
-    #one = 1;
-    get one() {
-        return this.#one;
-    }
-    set one(_) {
-        this.#one = _;
-    }
-    #_two = 2;
-    get #two() {
-        return this.#_two;
-    }
-    set #two(_) {
-        this.#_two = _;
-    }
-    #a = 3;
-    get [_b = three()]() {
-        return this.#a;
-    }
-    set [_b](_) {
-        this.#a = _;
-    }
-    static #four = 4;
-    static get four() {
-        return this.#four;
-    }
-    static set four(_) {
-        this.#four = _;
-    }
-    static #_five = 5;
-    static get #five() {
-        return this.#_five;
-    }
-    static set #five(_) {
-        this.#_five = _;
-    }
-    static #b = 6;
-    static get [_a = six()]() {
-        return this.#b;
-    }
-    static set [_a](_) {
-        this.#b = _;
-    }
-}
-class Normal {
-    constructor() {
-        __privateAdd(this, _a2, b);
-        this.c = d;
-    }
-    get a() {
-        return __privateGet(this, _a2);
-    }
-    set a(_) {
-        __privateSet(this, _a2, _);
-    }
-}
-_a2 = new WeakMap();
-class Private {
-    constructor() {
-        __privateAdd(this, __a, b);
-        this.c = d;
-    }
-    get #a() {
-        return __privateGet(this, __a);
-    }
-    set #a(_) {
-        __privateSet(this, __a, _);
-    }
-}
-__a = new WeakMap();
-class StaticNormal {
-    static #a = b;
-    static get a() {
-        return this.#a;
-    }
-    static set a(_) {
-        this.#a = _;
-    }
-    static {
-        this.c = d;
-    }
-}
-class StaticPrivate {
-    static #_a = b;
-    static get #a() {
-        return this.#_a;
-    }
-    static set #a(_) {
-        this.#_a = _;
-    }
-    static {
-        this.c = d;
-    }
-}

```