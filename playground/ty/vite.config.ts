import { defineConfig, type Plugin, searchForWorkspaceRoot } from "vite";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { readFileSync } from "fs";
import { dirname, join } from "path";
import { fileURLToPath } from "url";
import { normalizePath } from "vite";
import { viteStaticCopy } from "vite-plugin-static-copy";
import { parseExamples } from "./src/parseExamples";
import { DEFAULT_SETTINGS } from "./src/settings";

const PYODIDE_EXCLUDE = [
  "!**/*.{md,html}",
  "!**/*.d.ts",
  "!**/*.whl",
  "!**/node_modules",
];

// The playground examples are an mdtest suite, outside of the Vite root.
const EXAMPLES_DIR = normalizePath(
  fileURLToPath(
    new URL(
      "../../crates/ty_python_semantic/resources/mdtest/type_manipulation",
      import.meta.url,
    ),
  ),
);

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
    viteStaticCopyPyodide(),
    validateExamples(),
  ],
  optimizeDeps: { exclude: ["pyodide", "ty_wasm"] },
  server: {
    fs: {
      allow: [searchForWorkspaceRoot(process.cwd()), EXAMPLES_DIR],
    },
  },
});

/**
 * Parses `examples.md` whenever it's loaded, so that a malformed example fails
 * the build (or shows the dev server's error overlay) instead of surfacing as a
 * runtime error in the browser.
 */
function validateExamples(): Plugin {
  return {
    name: "ty-playground-validate-examples",
    transform(_code, id) {
      const [path] = id.split("?");
      if (normalizePath(path) === `${EXAMPLES_DIR}/examples.md`) {
        parseExamples(readFileSync(path, "utf8"), DEFAULT_SETTINGS);
      }
      return null;
    },
  };
}

export function viteStaticCopyPyodide() {
  const pyodideDir = normalizePath(
    join(dirname(fileURLToPath(import.meta.resolve("pyodide"))), "*"),
  );
  return viteStaticCopy({
    targets: [
      {
        src: [pyodideDir, ...PYODIDE_EXCLUDE],
        dest: "assets",
        rename: { stripBase: true },
      },
    ],
  });
}
