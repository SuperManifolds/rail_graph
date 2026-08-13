#!/usr/bin/env node
// Appends a `build_id` custom section to Trunk's output wasm so Sentry can
// symbolicate wasm stack frames. @sentry/wasm reads this section at runtime to
// derive the debug_id it reports (see static/sentry_wasm.min.js / sentry_bridge.js),
// and `sentry-cli debug-files upload` reads the SAME section from the uploaded
// copy — because we ship and upload the same file, the ids match and the DWARF
// (retained via index.html data-keep-debug) resolves frames to function + line.
//
// The id is the sha256 of the wasm (first 16 bytes), so it is deterministic per
// build and idempotent. Run from `beforeBuildCommand` (tauri build only, never
// `trunk serve`), so the dev workflow needs no node.
//
// Usage: node scripts/inject-wasm-build-id.mjs <dist-dir>

import { readFileSync, writeFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { createHash } from 'node:crypto';

const BUILD_ID_SECTION = 'build_id';
const BUILD_ID_BYTES = 16;

/** Unsigned LEB128 encoding of a non-negative integer. */
function leb128(value) {
  const out = [];
  let n = value;
  do {
    let byte = n & 0x7f;
    n >>>= 7;
    if (n !== 0) {
      byte |= 0x80;
    }
    out.push(byte);
  } while (n !== 0);
  return Buffer.from(out);
}

/** True if the wasm already carries a `build_id` custom section. */
function hasBuildId(bytes) {
  try {
    const module = new WebAssembly.Module(bytes);
    return WebAssembly.Module.customSections(module, BUILD_ID_SECTION).length > 0;
  } catch (err) {
    // If it will not compile here it would not run in the webview either; let the
    // real build surface that. Treat as "no build_id" so we still try to inject.
    console.warn(`inject-wasm-build-id: could not parse wasm (${err.message}); injecting anyway`);
    return false;
  }
}

/** Append a `build_id` custom section: id(0x00) + size + nameLen + name + id. */
function withBuildId(bytes) {
  const id = createHash('sha256').update(bytes).digest().subarray(0, BUILD_ID_BYTES);
  const name = Buffer.from(BUILD_ID_SECTION, 'utf8');
  const content = Buffer.concat([leb128(name.length), name, id]);
  const section = Buffer.concat([Buffer.from([0x00]), leb128(content.length), content]);
  return { wasm: Buffer.concat([bytes, section]), id: id.toString('hex') };
}

function main() {
  const dir = process.argv[2];
  if (!dir) {
    console.error('usage: inject-wasm-build-id.mjs <dist-dir>');
    process.exit(1);
  }
  const wasmFiles = readdirSync(dir).filter((f) => f.endsWith('_bg.wasm'));
  if (wasmFiles.length === 0) {
    console.error(`inject-wasm-build-id: no *_bg.wasm found in ${dir}`);
    process.exit(1);
  }
  for (const file of wasmFiles) {
    const path = join(dir, file);
    const bytes = readFileSync(path);
    if (hasBuildId(bytes)) {
      console.log(`inject-wasm-build-id: ${file} already has a build_id; skipping`);
      continue;
    }
    const { wasm, id } = withBuildId(bytes);
    writeFileSync(path, wasm);
    console.log(`inject-wasm-build-id: injected build_id ${id} into ${file}`);
  }
}

main();
