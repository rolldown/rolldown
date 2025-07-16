import "./dep-a.js"; // This incorrectly executes after dep-b
import "./dep-b.js"; // This incorrectly executes before dep-a

