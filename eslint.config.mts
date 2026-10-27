import js from "@eslint/js";
import { defineConfig } from "eslint/config";
import globals from "globals";
import tseslint from "typescript-eslint";

const webFiles = ["sow-web/**/*.ts"];
const toolFiles = ["sow-tools/**/*.ts", "sow-tools/**/*.mts"];
const shellGlobals = {
  SOW_t: "readonly",
  SOW_renderDropdown: "readonly",
  module: "writable",
};

export default defineConfig([
  {
    ignores: [
      "**/node_modules/**",
      "**/target/**",
      "**/dist/**",
      "sow-web/shell/main_menu.core.ts",
      "sow-web/shell/main_menu.motion.ts",
      "sow-web/shell/main_menu.lobbies.ts",
      "sow-web/shell/main_menu.store.ts",
      "sow-web/shell/main_menu.heroes.ts",
      "sow-web/shell/main_menu.profile.ts",
      "sow-web/shell/main_menu.shell.ts",
      "sow-web/shell/main_menu.poki.ts",
      "sow-web/shell/main_menu.jest.ts",
    ],
  },
  {
    files: [...webFiles, ...toolFiles],
    extends: [js.configs.recommended, tseslint.configs.recommended],
    rules: {
      eqeqeq: ["error", "always", { null: "ignore" }],
      "no-empty": ["error", { allowEmptyCatch: true }],
      "no-var": "error",
      "prefer-const": "error",
      "@typescript-eslint/no-unused-vars": ["error", { argsIgnorePattern: "^_" }],
    },
  },
  {
    files: webFiles,
    languageOptions: {
      globals: globals.browser,
    },
  },
  {
    files: ["sow-web/**/*.test.ts", "sow-web/shell/typescript-test-support.ts"],
    languageOptions: {
      globals: globals.node,
    },
  },
  {
    files: ["sow-web/shell/**/*.ts"],
    languageOptions: {
      globals: shellGlobals,
    },
    rules: {
      "no-undef": "off",
    },
  },
  {
    files: toolFiles,
    languageOptions: {
      globals: { ...globals.browser, ...globals.node },
    },
  },
]);
