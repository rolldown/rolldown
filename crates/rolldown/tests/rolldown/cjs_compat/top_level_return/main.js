// Test case showing that top-level return exits early
exports.before = "before return";
return "foo";
exports.after = "after return"; // This should not be executed