(() => {
	function a() {
		b()
	}
	{
		var b = () => {console.log()}
	}
	a()
})()