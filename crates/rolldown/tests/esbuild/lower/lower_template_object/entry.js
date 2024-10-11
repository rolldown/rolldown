const x = () => [tag`x`, tag`\\xFF`, tag`\\x`, tag`\\u`];
const y = () => [
	tag`x\${y}z`,
	tag`\\xFF\${y}z`,
	tag`x\${y}\\z`,
	tag`x\${y}\\u`,
];
