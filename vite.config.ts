import fs from 'node:fs';
import path from 'node:path';
import type { Plugin } from 'vite';
import { defineConfig } from 'vitest/config';
import { patchFighterSource } from './src/editor/source.ts';

/** Dev-only endpoint the hitbox editor posts to: rewrites moves in src/data/fighters/<id>.ts (HMR then reloads them). */
function editorSave(): Plugin {
  return {
    name: 'stick-smash-editor-save',
    apply: 'serve',
    configureServer(server) {
      server.middlewares.use('/__editor/save', (req, res) => {
        const reply = (code: number, body: object) => { res.statusCode = code; res.setHeader('content-type', 'application/json'); res.end(JSON.stringify(body)); };
        if (req.method !== 'POST') return reply(405, { ok: false, error: 'POST only' });
        let body = '';
        req.on('data', c => { body += c; });
        req.on('end', () => {
          try {
            const { fighter, moves } = JSON.parse(body);
            if (!/^[a-z0-9_-]+$/i.test(fighter)) throw new Error(`bad fighter id ${fighter}`);
            const file = path.join(server.config.root, 'src/data/fighters', `${fighter}.ts`);
            const src = fs.readFileSync(file, 'utf8'), out = patchFighterSource(src, moves);
            if (out !== src) fs.writeFileSync(file, out);
            reply(200, { ok: true, file: path.relative(server.config.root, file) });
          } catch (e) {
            reply(400, { ok: false, error: String(e instanceof Error ? e.message : e) });
          }
        });
      });
    },
  };
}

export default defineConfig({
  base: './',
  plugins: [editorSave()],
  server: { open: true },
  build: { chunkSizeWarningLimit: 2000 },
  test: { include: ['tests/**/*.test.ts'] },
});
