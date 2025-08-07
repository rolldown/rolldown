this.a = 2
assert(window.a, 2)
function a() {
    this.b = 3
}

a()