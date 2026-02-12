import { createRequire } from 'module';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import fs from 'fs';

export function loadCoreApi(): any {
  const __filename = fileURLToPath(import.meta.url);
  const __dirname = dirname(__filename);
  const require = createRequire(import.meta.url);

  const candidates = [
    join(__dirname, '../carrycode-coreapi.linux-x64-gnu.node'),
    join(__dirname, '../../carrycode-coreapi.linux-x64-gnu.node'),
    join(__dirname, './carrycode-coreapi.linux-x64-gnu.node'),
    join(__dirname, '../../target/carrycode-coreapi.linux-x64-gnu.node'),
    join(__dirname, '../target/carrycode-coreapi.linux-x64-gnu.node'),
  ];

  for (const p of candidates) {
    if (fs.existsSync(p)) {
      return require(p);
    }
  }

  throw new Error(
    `Native module not found. Tried:\n` +
      candidates.map((c) => ` - ${c}`).join('\n') +
      `\nPlease run: ./node_modules/.bin/napi build --platform --release`,
  );
}
