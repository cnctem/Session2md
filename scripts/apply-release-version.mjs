#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const rootDir = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
const tag = (process.argv[2] ?? process.env.GITHUB_REF_NAME)?.trim();
const tagMatch =
  /^v?(\d+\.\d+\.\d+(?:-[0-9A-Za-z]+(?:[.-][0-9A-Za-z]+)*)?(?:\+[0-9A-Za-z]+(?:[.-][0-9A-Za-z]+)*)?)$/.exec(
    tag ?? "",
  );

if (!tagMatch) {
  console.error(
    "Usage: node scripts/apply-release-version.mjs <tag>\n" +
      "The tag must be a SemVer value such as 2.3.0 or v2.3.0.",
  );
  process.exit(1);
}

const version = tagMatch[1];

async function updateJson(relativePath, update) {
  const filePath = path.join(rootDir, relativePath);
  const data = JSON.parse(await readFile(filePath, "utf8"));
  update(data);
  await writeFile(filePath, `${JSON.stringify(data, null, 2)}\n`);
}

function replaceRequired(relativePath, content, pattern, replacement) {
  if (!pattern.test(content)) {
    throw new Error(`Unable to find the version field in ${relativePath}`);
  }

  return content.replace(pattern, replacement);
}

async function main() {
  await updateJson("package.json", (data) => {
    data.version = version;
  });
  await updateJson("src-tauri/tauri.conf.json", (data) => {
    data.version = version;
  });

  const cargoTomlPath = path.join(rootDir, "src-tauri/Cargo.toml");
  const cargoToml = await readFile(cargoTomlPath, "utf8");
  await writeFile(
    cargoTomlPath,
    replaceRequired(
      "src-tauri/Cargo.toml",
      cargoToml,
      /^version = "[^"]+"$/m,
      `version = "${version}"`,
    ),
  );

  const cargoLockPath = path.join(rootDir, "src-tauri/Cargo.lock");
  const cargoLock = await readFile(cargoLockPath, "utf8");
  await writeFile(
    cargoLockPath,
    replaceRequired(
      "src-tauri/Cargo.lock",
      cargoLock,
      /(\[\[package\]\]\nname = "session2md"\nversion = ")[^"]+("\n)/,
      `$1${version}$2`,
    ),
  );

  console.log(
    `[release-version] Applied ${version} from release tag ${tag} to package.json, tauri.conf.json, Cargo.toml, and Cargo.lock.`,
  );
}

main().catch((error) => {
  console.error(`[release-version] ${error.message}`);
  process.exit(1);
});
