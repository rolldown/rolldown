function Foo() {}

let a = <div/>
let b = <Foo>{a}</Foo>
let c = <>{b}</>

let d = <div/>
let e = <Foo>{d}</Foo>
let f = <>{e}</>
console.log(f)