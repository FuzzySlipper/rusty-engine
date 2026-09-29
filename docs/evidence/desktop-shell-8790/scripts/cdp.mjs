// Evaluate one expression in the shell's page over CDP and print the JSON result.
// usage: node cdp.mjs <cdp-origin> <expression>
import { createRequire } from 'node:module';
const require = createRequire('/home/agent/dev/crew-services/internal/playtest/browser/');
const { chromium } = require('playwright-core');

const [origin, expression] = process.argv.slice(2);
const browser = await chromium.connectOverCDP(origin);
const page = browser.contexts().flatMap((context) => context.pages())
  .find((candidate) => !candidate.url().startsWith('devtools:'));
const value = await page.evaluate(expression);
console.log(JSON.stringify(value));
await browser.close().catch(() => undefined);
process.exit(0);
