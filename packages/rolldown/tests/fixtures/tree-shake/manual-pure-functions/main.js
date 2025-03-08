import styled from 'styled-components';
let local = console.log;
local(); // removed
styled.div`
	color: blue;
`; // removed
styled.div; // removed
styled?.div(); // removed
styled()(); // removed
styled().div(); // removed

let another = console.log;
another(); // retained

