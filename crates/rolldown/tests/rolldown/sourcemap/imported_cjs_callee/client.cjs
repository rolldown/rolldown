module.exports.createRoot = function createRoot(element) {
  return {
    render() {
      element.textContent = 'rendered';
    },
  };
};
