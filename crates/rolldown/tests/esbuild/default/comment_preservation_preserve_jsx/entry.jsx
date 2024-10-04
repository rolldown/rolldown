console.log(
	<div x={/*before*/x} />,
	<div x={/*before*/'y'} />,
	<div x={/*before*/true} />,
	<div {/*before*/...x} />,
	<div>{/*before*/x}</div>,
	<>{/*before*/x}</>,

	// Comments on absent AST nodes
	<div>before{}after</div>,
	<div>before{/* comment 1 *//* comment 2 */}after</div>,
	<div>before{
		// comment 1
		// comment 2
	}after</div>,
	<>before{}after</>,
	<>before{/* comment 1 *//* comment 2 */}after</>,
	<>before{
		// comment 1
		// comment 2
	}after</>,
)