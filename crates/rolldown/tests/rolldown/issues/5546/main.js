// This test reproduces the antd bundle output issue where duplicate
// exports.default statements are not properly eliminated by dead code elimination

Object.defineProperty(exports, "__esModule", { 
  value: true
});

const localeValues = {
  placeholder: "Selecione uma opção",
};

// First export.default statement  
exports.default = localeValues;

// Dead code that should be eliminated - unreachable conditional
if (false) {
  exports.default = null;
}

// Conditional that evaluates to true but creates duplicate assignment
if (typeof exports === 'object' && exports.__esModule) {
  exports.default = localeValues; // This should be detected as redundant
}