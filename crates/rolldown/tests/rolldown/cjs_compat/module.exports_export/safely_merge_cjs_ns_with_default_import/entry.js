// Test case for SafelyMergeCjsNs optimization with both named and default imports
import React, { useState } from 'this-is-only-used-for-testing';

console.log(React);
console.log(React.default);
console.log(useState);
