x = () => {
	class XYYYYY {} // This should be named "Y" due to frequency analysis
	return <XYYYYY tag-must-start-with-capital-letter />
}