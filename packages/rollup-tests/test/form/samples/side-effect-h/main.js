function foo ( ok ) {
	if ( !ok ) {
		throw new Error( 'this will be ignored' );
	}
}

foo();

export default 42;
