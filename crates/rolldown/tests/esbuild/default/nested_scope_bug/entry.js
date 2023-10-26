(() => {
	function a() {
		b()
	}
	{
		var b = () => {}
	}
	a()
})()