let Foo = {
	Bar_(props) {
		return <>{props.text_}</>
	},
	hello_: 'hello, world',
	createElement_(...args) {
		console.log('createElement', ...args)
	},
	Fragment_(...args) {
		console.log('Fragment', ...args)
	},
}
export default <Foo.Bar_ text_={Foo.hello_}></Foo.Bar_>