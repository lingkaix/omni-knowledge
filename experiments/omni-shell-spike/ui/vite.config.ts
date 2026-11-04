import { transform } from "esbuild";
import { fileURLToPath, URL } from "node:url";
import react from "@vitejs/plugin-react";
import { defineConfig, type Plugin } from "vite";

const knowledgeUi = fileURLToPath(
  new URL(
    "../vendor/openkit-2f031885/packages/knowledge-ui/src/index.ts",
    import.meta.url,
  ),
);
const modules = new URL("./node_modules/", import.meta.url);
const moduleFile = (relative: string) => fileURLToPath(new URL(relative, modules));

/**
 * Vite loads the nearest tsconfig for every TypeScript file and fails when
 * that file extends a path this snapshot does not contain. The package bytes
 * stay unchanged. This plugin transpiles only that snapshot.
 */
function vendoredKnowledgeUi(): Plugin {
  return {
    name: "vendored-knowledge-ui",
    enforce: "pre",
    async transform(code, id) {
      const file = id.split("?", 1)[0];
      if (!file.includes("/openkit-2f031885/") || !/\.tsx?$/.test(file)) return null;
      const result = await transform(code, {
        loader: file.endsWith("tsx") ? "tsx" : "ts",
        format: "esm",
        jsx: "automatic",
        target: "esnext",
        sourcefile: file,
        tsconfigRaw: "{}",
      });
      return { code: result.code, map: result.map || null };
    },
  };
}

export default defineConfig({
  plugins: [
    vendoredKnowledgeUi(),
    react({ exclude: /openkit-2f031885/ }),
  ],
  esbuild: {
    exclude: /openkit-2f031885/,
  },
  resolve: {
    alias: [
      { find: "@openkit/knowledge-ui", replacement: knowledgeUi },
      // The snapshot sits outside this package, so Node walks past ui/node_modules.
      { find: "react/jsx-dev-runtime", replacement: moduleFile("react/jsx-dev-runtime.js") },
      { find: "react/jsx-runtime", replacement: moduleFile("react/jsx-runtime.js") },
      { find: "react-dom/client", replacement: moduleFile("react-dom/client.js") },
      { find: "react-dom", replacement: moduleFile("react-dom/index.js") },
      {
        find: "react-aria-components",
        replacement: moduleFile("react-aria-components/dist/exports/index.mjs"),
      },
      { find: "react", replacement: moduleFile("react/index.js") },
    ],
  },
  build: {
    outDir: "dist",
    assetsDir: ".",
    emptyOutDir: true,
    rollupOptions: {
      output: {
        entryFileNames: "main.js",
        chunkFileNames: "chunk-[name].js",
        assetFileNames: "asset-[name][extname]",
      },
    },
  },
});
