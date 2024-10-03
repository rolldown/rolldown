if (shouldBeExportsNotThis) {
	console.log(this)
	console.log((x = this) => this)
	console.log({x: this})
	console.log(class extends this.foo {})
	console.log(class { [this.foo] })
	console.log(class { [this.foo]() {} })
	console.log(class { static [this.foo] })
	console.log(class { static [this.foo]() {} })
}
if (shouldBeThisNotExports) {
	console.log(class { foo = this })
	console.log(class { foo() { this } })
	console.log(class { static foo = this })
	console.log(class { static foo() { this } })
}