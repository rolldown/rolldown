# Diff
## /out.js
### esbuild
```js
class Foo {
  #x;
  unary() {
    this.#x++;
    this.#x--;
    ++this.#x;
    --this.#x;
  }
  binary() {
    this.#x = 1;
    this.#x += 1;
    this.#x -= 1;
    this.#x *= 1;
    this.#x /= 1;
    this.#x %= 1;
    this.#x **= 1;
    this.#x <<= 1;
    this.#x >>= 1;
    this.#x >>>= 1;
    this.#x &= 1;
    this.#x |= 1;
    this.#x ^= 1;
    this.#x &&= 1;
    this.#x ||= 1;
    this.#x ??= 1;
  }
}
```
### rolldown
```js


```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,27 +0,0 @@
-class Foo {
-    #x;
-    unary() {
-        this.#x++;
-        this.#x--;
-        ++this.#x;
-        --this.#x;
-    }
-    binary() {
-        this.#x = 1;
-        this.#x += 1;
-        this.#x -= 1;
-        this.#x *= 1;
-        this.#x /= 1;
-        this.#x %= 1;
-        this.#x **= 1;
-        this.#x <<= 1;
-        this.#x >>= 1;
-        this.#x >>>= 1;
-        this.#x &= 1;
-        this.#x |= 1;
-        this.#x ^= 1;
-        this.#x &&= 1;
-        this.#x ||= 1;
-        this.#x ??= 1;
-    }
-}

```