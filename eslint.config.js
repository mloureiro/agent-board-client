import js from '@eslint/js';
import { defineConfig, globalIgnores } from 'eslint/config';
import tseslint from 'typescript-eslint';

export default defineConfig(
  globalIgnores(['dist/', 'src-tauri/target/', 'src-tauri/gen/']),
  js.configs.recommended,
  tseslint.configs.strictTypeChecked,
  {
    languageOptions: {
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
  },
  {
    files: ['src-tauri/scripts/*.js'],
    extends: [tseslint.configs.disableTypeChecked],
    rules: { '@typescript-eslint/no-unused-expressions': 'off' },
    languageOptions: {
      sourceType: 'script',
      globals: { window: 'readonly', location: 'readonly', Element: 'readonly', URL: 'readonly' },
    },
  },
);
