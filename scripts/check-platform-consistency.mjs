import { readFile } from "node:fs/promises";

const expectedVersion = "14.0";
const expectedSwiftVersion = ".macOS(.v14)";
const expectedRunner = "runs-on: macos-14";
const failures = [];

async function requireText(path, expected, description) {
  const content = await readFile(path, "utf8");
  if (!content.includes(expected)) {
    failures.push(`${path}: expected ${description} (${JSON.stringify(expected)})`);
  }
}

const tauriConfig = JSON.parse(
  await readFile("src-tauri/tauri.conf.json", "utf8"),
);
const configuredVersion = tauriConfig.bundle?.macOS?.minimumSystemVersion;
if (configuredVersion !== expectedVersion) {
  failures.push(
    `src-tauri/tauri.conf.json: minimumSystemVersion is ${JSON.stringify(configuredVersion)}; expected ${JSON.stringify(expectedVersion)}`,
  );
}

const capability = JSON.parse(
  await readFile("src-tauri/capabilities/default.json", "utf8"),
);
if (!capability.windows?.includes("sidebar") || !capability.permissions?.includes("core:event:default")) {
  failures.push("src-tauri/capabilities/default.json: sidebar must be allowed to listen for file and state events");
}

await Promise.all([
  requireText(
    "src-tauri/darwinkit/Package.swift",
    expectedSwiftVersion,
    "the Swift deployment target",
  ),
  requireText("README.md", "macOS 14+", "the public support requirement"),
  requireText("README.md", "Apple Silicon", "the Apple Silicon architecture target"),
  requireText(
    "CONTRIBUTING.md",
    "| macOS | 14+ |",
    "the contributor support requirement",
  ),
  requireText(
    ".github/workflows/ci.yml",
    expectedRunner,
    "the macOS CI runner",
  ),
  requireText(
    ".github/workflows/release.yml",
    "runs-on: macos-15",
    "the release runner",
  ),
  requireText("scripts/build-dev.sh", "supports Apple Silicon Macs only", "the local build architecture restriction"),
  requireText(
    ".github/workflows/beta.yml",
    "runs-on: macos-15",
    "the beta runner",
  ),
  ...["beta", "release"].flatMap((workflow) => [
    requireText(`.github/workflows/${workflow}.yml`, "uses: ./.github/workflows/ci.yml", "the shared quality gates"),
    requireText(`.github/workflows/${workflow}.yml`, "needs: checks", "verification before publication"),
  ]),
  ...["ci", "beta", "release"].map((workflow) =>
    requireText(`.github/workflows/${workflow}.yml`, "DEVELOPER_DIR: /Applications/Xcode_26.3.app/Contents/Developer", "a compatible pinned Swift build toolchain"),
  ),
  requireText(".github/workflows/beta.yml", '--target "$GITHUB_SHA"', "the exact beta build revision"),
  requireText(".github/workflows/release.yml", "Sidebar Notes.app", "the fork's app bundle name"),
  requireText(".github/workflows/release.yml", "--target aarch64-apple-darwin", "the Apple Silicon release target"),
]);

const releaseWorkflow = await readFile(".github/workflows/release.yml", "utf8");
if (/x86_64|0xMassi\/stik_app|HOMEBREW_TAP_TOKEN|VERCEL_DEPLOY_HOOK/.test(releaseWorkflow)) {
  failures.push(".github/workflows/release.yml: release must target this fork on Apple Silicon only");
}

const ciWorkflow = await readFile(".github/workflows/ci.yml", "utf8");
if (/x86_64|macos-15-intel/.test(ciWorkflow)) {
  failures.push(".github/workflows/ci.yml: remove Intel-only build targets");
}

if (failures.length > 0) {
  console.error("Release configuration checks failed:\n");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log("macOS 14 (Sonoma) support and release safety declarations are consistent.");
