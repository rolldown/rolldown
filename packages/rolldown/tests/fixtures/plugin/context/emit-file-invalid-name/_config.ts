import { defineTest } from "rolldown-tests";
import { expect } from "vitest";

// Test that emitFile rejects invalid asset names (absolute/relative paths)
// This matches Rollup's behavior
export default defineTest({
  config: {
    plugins: [
      {
        name: "test-plugin",
        buildStart() {
          // Test relative path starting with "./"
          expect(() => {
            this.emitFile({
              type: "asset",
              name: "./relative.txt",
              source: "content",
            });
          }).toThrow(
            'The "fileName" or "name" properties of emitted chunks and assets must be strings that are neither absolute nor relative paths',
          );

          // Test relative path starting with "../"
          expect(() => {
            this.emitFile({
              type: "asset",
              name: "../parent.txt",
              source: "content",
            });
          }).toThrow(
            'The "fileName" or "name" properties of emitted chunks and assets must be strings that are neither absolute nor relative paths',
          );

          // Test absolute Unix path
          expect(() => {
            this.emitFile({
              type: "asset",
              name: "/absolute.txt",
              source: "content",
            });
          }).toThrow(
            'The "fileName" or "name" properties of emitted chunks and assets must be strings that are neither absolute nor relative paths',
          );

          // Test Windows absolute path (only on Windows)
          if (process.platform === "win32") {
            expect(() => {
              this.emitFile({
                type: "asset",
                name: "C:/windows.txt",
                source: "content",
              });
            }).toThrow(
              'The "fileName" or "name" properties of emitted chunks and assets must be strings that are neither absolute nor relative paths',
            );
          }

          // Emit a valid asset so the build succeeds
          this.emitFile({
            type: "asset",
            name: "valid.txt",
            source: "valid content",
          });
        },
      },
    ],
  },
});
