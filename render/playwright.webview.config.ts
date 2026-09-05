import { defineConfig } from '@playwright/test';
import browserConfig from './playwright.config.js';

export default defineConfig(browserConfig, {
  testMatch: '**/webview-artifact.optional.spec.ts',
});
