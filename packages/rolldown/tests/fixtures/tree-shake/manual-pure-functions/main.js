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

function effect(value) {
  console.log(value);
  return value;
}
styled()[effect('computed key')]; // retained
styled(effect('call argument')).value; // retained
new (styled()[effect('new callee')].Box)(); // retained

let another = console.log;
another(); // retained
