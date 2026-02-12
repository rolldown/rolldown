// Test that </script> is escaped to <\/script> in (tagged) template literals
export const raw = String.raw`</script>`;
export const template = `</script>`;
