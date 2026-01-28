import eslint from '@eslint/js';
import tseslint from '@typescript-eslint/eslint-plugin';
import tsParser from '@typescript-eslint/parser';
import prettierConfig from 'eslint-config-prettier';
import { fileURLToPath } from 'url';
import { dirname } from 'path';

const __dirname = dirname(fileURLToPath(import.meta.url));

export default [
  eslint.configs.recommended,
  {
    files: ['signet-program/**/*.ts', 'signet-program/**/*.tsx'],
    languageOptions: {
      parser: tsParser,
      parserOptions: {
        project: './signet-program/tsconfig.json',
        tsconfigRootDir: __dirname,
      },
      globals: {
        Buffer: 'readonly',
        console: 'readonly',
        process: 'readonly',
        module: 'readonly',
        __dirname: 'readonly',
        __filename: 'readonly',
        setTimeout: 'readonly',
        clearTimeout: 'readonly',
        describe: 'readonly',
        it: 'readonly',
        before: 'readonly',
        after: 'readonly',
        beforeEach: 'readonly',
        afterEach: 'readonly',
      },
    },
    plugins: {
      '@typescript-eslint': tseslint,
    },
    rules: {
      ...tseslint.configs.recommended.rules,
      '@typescript-eslint/no-explicit-any': 'error',
      '@typescript-eslint/no-unused-vars': [
        'error',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_' },
      ],
      'no-console': ['warn', { allow: ['warn', 'error'] }],
    },
  },
  {
    files: ['**/*.spec.ts', '**/*.test.ts'],
    rules: {
      '@typescript-eslint/no-explicit-any': 'warn',
    },
  },
  prettierConfig,
  {
    ignores: [
      '**/target/**',
      '**/node_modules/**',
      '**/dist/**',
      '**/*.d.ts',
      'clients/**',
    ],
  },
];