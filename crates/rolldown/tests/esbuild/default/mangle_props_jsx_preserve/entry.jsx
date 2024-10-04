let Foo = {
	Bar_(props) {
		return <>{props.text_}</>
	},
	hello_: 'hello, world',
}
export default <Foo.Bar_ text_={Foo.hello_}></Foo.Bar_>