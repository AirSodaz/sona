import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';
import https from 'https';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

/** The version of Node.js to download and install. */
const NODE_VERSION = 'v22.22.0';

/**
 * Map of Node.js platform/arch combinations to Rust target triples.
 * Used to determine the correct binary suffix for the current platform.
 */
const TARGETS = {
  'win32-x64': 'x86_64-pc-windows-msvc',
  'linux-x64': 'x86_64-unknown-linux-gnu',
  'darwin-x64': 'x86_64-apple-darwin',
  'darwin-arm64': 'aarch64-apple-darwin',
};

/**
 * Determines the Rust target triple for the current platform.
 *
 * @return {string} The Rust target triple (e.g., 'x86_64-unknown-linux-gnu').
 */
function getTarget() {
  const platform = process.platform;
  const arch = process.arch;
  const key = `${platform}-${arch}`;
  return TARGETS[key] || key;
}

const binariesDir = path.resolve(__dirname, '../src-tauri/binaries');
const sidecarDir = path.resolve(__dirname, '../src-tauri/sidecar');

console.log(`Setup Sidecar:
  Node Version: ${NODE_VERSION}
  Platform: ${process.platform}
  Arch: ${process.arch}
`);

if (!fs.existsSync(binariesDir)) {
  fs.mkdirSync(binariesDir, { recursive: true });
}

/**
 * Downloads a file from a URL to a local destination.
 * Handles redirects (301/302) automatically.
 *
 * @param {string} url - The URL to download from.
 * @param {string} dest - The local file path to save the download to.
 * @return {Promise<void>} A promise that resolves when the download is complete.
 */
async function downloadFile(url, dest) {
  return new Promise((resolve, reject) => {
    const file = fs.createWriteStream(dest);
    https.get(url, (response) => {
      if (response.statusCode === 302 || response.statusCode === 301) {
        downloadFile(response.headers.location, dest).then(resolve).catch(reject);
        return;
      }
      if (response.statusCode !== 200) {
        reject(new Error(`Failed to download ${url}: ${response.statusCode}`));
        return;
      }
      response.pipe(file);
      file.on('finish', () => {
        file.close();
        resolve();
      });
    }).on('error', (err) => {
      fs.unlink(dest, () => { });
      reject(err);
    });
  });
}

/**
 * Sets up the Node.js binary for the sidecar.
 * Checks if the binary already exists; if not, downloads and extracts it.
 *
 * @param {string} targetTriple - The Rust target triple (e.g., 'x86_64-apple-darwin').
 * @return {Promise<void>} A promise that resolves when Node.js is set up.
 */
async function setupNode(targetTriple) {
  // Determine Node platform and arch from targetTriple
  let nodePlatform, nodeArch;

  if (targetTriple.includes('windows') || targetTriple.includes('win32')) {
    nodePlatform = 'win32';
    nodeArch = (targetTriple.includes('aarch64') || targetTriple.includes('arm64')) ? 'arm64' : 'x64';
  } else if (targetTriple.includes('apple-darwin') || targetTriple.includes('darwin')) {
    nodePlatform = 'darwin';
    nodeArch = (targetTriple.includes('aarch64') || targetTriple.includes('arm64')) ? 'arm64' : 'x64';
  } else if (targetTriple.includes('linux')) {
    nodePlatform = 'linux';
    nodeArch = (targetTriple.includes('aarch64') || targetTriple.includes('arm64')) ? 'arm64' : 'x64';
  } else {
    // Fallback to host platform if unknown target
    console.warn(`Unknown target triple pattern: ${targetTriple}, falling back to host defaults.`);
    nodePlatform = process.platform;
    nodeArch = process.arch === 'arm64' ? 'arm64' : 'x64';
  }

  const ext = nodePlatform === 'win32' ? '.exe' : '';
  const binaryName = `node-${targetTriple}${ext}`;
  const binaryPath = path.join(binariesDir, binaryName);

  if (fs.existsSync(binaryPath)) {
    console.log(`Node binary already exists at ${binaryPath}`);
    return;
  }

  console.log(`Downloading Node.js binary for ${targetTriple} (${nodePlatform}-${nodeArch}) to ${binaryPath}...`);

  if (nodePlatform === 'win32') {
    // Windows: Download node.exe directly
    const url = `https://nodejs.org/dist/${NODE_VERSION}/win-x64/node.exe`;
    await downloadFile(url, binaryPath);
  } else {
    // Unix: Download tar.gz and extract
    const archiveName = `node-${NODE_VERSION}-${nodePlatform}-${nodeArch}.tar.gz`;
    const url = `https://nodejs.org/dist/${NODE_VERSION}/${archiveName}`;
    const tarPath = path.join(binariesDir, archiveName);

    await downloadFile(url, tarPath);

    console.log(`Extracting Node binary (${archiveName})...`);

    try {
      // Create unique temp dir for this target to avoid conflicts
      const tmpDir = path.join(binariesDir, `tmp_${targetTriple}`);
      if (fs.existsSync(tmpDir)) fs.rmSync(tmpDir, { recursive: true, force: true });
      fs.mkdirSync(tmpDir);

      execSync(`tar -xf "${tarPath}" -C "${tmpDir}"`);

      // Find the node binary in tmpDir
      const findNode = (dir) => {
        const entries = fs.readdirSync(dir, { withFileTypes: true });
        for (const entry of entries) {
          if (entry.isDirectory()) {
            const found = findNode(path.join(dir, entry.name));
            if (found) return found;
          } else if (entry.name === 'node') {
            return path.join(dir, entry.name);
          }
        }
        return null;
      };

      const extractedNode = findNode(tmpDir);
      if (extractedNode) {
        fs.copyFileSync(extractedNode, binaryPath);
        fs.chmodSync(binaryPath, 0o755); // Make executable
      } else {
        throw new Error(`Could not find node binary in extracted archive for ${targetTriple}`);
      }

      // Cleanup
      fs.rmSync(tmpDir, { recursive: true, force: true });
      fs.unlinkSync(tarPath);

    } catch (e) {
      console.error(`Error extracting node for ${targetTriple}:`, e);
      process.exit(1);
    }
  }
  console.log(`Node binary setup complete for ${targetTriple}.`);
}

/**
 * Installs dependencies and builds the sidecar application.
 *
 * @return {Promise<void>} A promise that resolves when the sidecar is built.
 */
async function installSidecarDeps() {
  console.log('Installing sidecar dependencies...');
  try {
    execSync('npm install', {
      cwd: sidecarDir,
      stdio: 'inherit',
      env: { ...process.env } // Ensure path and other env vars are passed
    });
    console.log('Sidecar dependencies installed.');

    console.log('Building sidecar...');
    execSync('npm run build', {
      cwd: sidecarDir,
      stdio: 'inherit',
      env: { ...process.env }
    });
    console.log('Sidecar built.');

  } catch (e) {
    console.error('Failed to install/build sidecar dependencies:', e);
    process.exit(1);
  }
}

/**
 * Main execution function.
 */
async function main() {
  const currentTarget = process.argv[2] || getTarget();

  if (process.platform === 'darwin') {
    // On macOS, download both architectures to support Universal builds
    console.log('Detected macOS platform. Setting up both x86_64 and aarch64 binaries for Universal build support.');
    await setupNode('x86_64-apple-darwin');
    await setupNode('aarch64-apple-darwin');
  } else {
    // Other platforms: setup only the requested/detected target
    await setupNode(currentTarget);
  }

  await installSidecarDeps();
}

main().catch(err => {
  console.error(err);
  process.exit(1);
});
