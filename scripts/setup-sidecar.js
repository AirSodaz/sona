import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';
import https from 'https';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const NODE_VERSION = 'v20.18.0';

// Map Node.js platform/arch to Rust target triple
const TARGETS = {
  'win32-x64': 'x86_64-pc-windows-msvc',
  'linux-x64': 'x86_64-unknown-linux-gnu',
  'darwin-x64': 'x86_64-apple-darwin',
  'darwin-arm64': 'aarch64-apple-darwin',
};

function getTarget() {
  const platform = process.platform;
  const arch = process.arch;
  const key = `${platform}-${arch}`;
  return TARGETS[key] || key;
}

const targetTriple = process.argv[2] || getTarget();
const binariesDir = path.resolve(__dirname, '../src-tauri/binaries');
const sidecarDir = path.resolve(__dirname, '../src-tauri/sidecar');

console.log(`Setup Sidecar:
  Node Version: ${NODE_VERSION}
  Platform: ${process.platform}
  Arch: ${process.arch}
  Target Triple: ${targetTriple}
`);

if (!fs.existsSync(binariesDir)) {
  fs.mkdirSync(binariesDir, { recursive: true });
}

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
      fs.unlink(dest, () => {});
      reject(err);
    });
  });
}

async function setupNode() {
  const ext = process.platform === 'win32' ? '.exe' : '';
  const binaryName = `node-${targetTriple}${ext}`;
  const binaryPath = path.join(binariesDir, binaryName);

  if (fs.existsSync(binaryPath)) {
    console.log(`Node binary already exists at ${binaryPath}`);
  } else {
    console.log(`Downloading Node.js binary to ${binaryPath}...`);

    if (process.platform === 'win32') {
      // Windows: Download node.exe directly
      const url = `https://nodejs.org/dist/${NODE_VERSION}/win-x64/node.exe`;
      await downloadFile(url, binaryPath);
    } else {
      // Unix: Download tar.gz and extract
      const platform = process.platform === 'darwin' ? 'darwin' : 'linux';
      const arch = process.arch === 'arm64' ? 'arm64' : 'x64'; // Node uses x64, not x86_64
      const archiveName = `node-${NODE_VERSION}-${platform}-${arch}.tar.gz`;
      const url = `https://nodejs.org/dist/${NODE_VERSION}/${archiveName}`;
      const tarPath = path.join(binariesDir, archiveName);

      await downloadFile(url, tarPath);

      console.log('Extracting Node binary...');
      // Extract specific file using tar
      // bin/node is the path inside the tarball
      // The tarball structure is node-v.../bin/node
      // We use --strip-components to flatten or direct extraction?
      // Simpler: extract to temp dir then move

      try {
        // Create temp dir
        const tmpDir = path.join(binariesDir, 'tmp_node');
        if (fs.existsSync(tmpDir)) fs.rmSync(tmpDir, { recursive: true, force: true });
        fs.mkdirSync(tmpDir);

        execSync(`tar -xf "${tarPath}" -C "${tmpDir}"`);

        // Find the node binary in tmpDir
        // Structure: tmpDir/node-v.../bin/node
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
          throw new Error('Could not find node binary in extracted archive');
        }

        // Cleanup
        fs.rmSync(tmpDir, { recursive: true, force: true });
        fs.unlinkSync(tarPath);

      } catch (e) {
        console.error('Error extracting node:', e);
        process.exit(1);
      }
    }
    console.log('Node binary setup complete.');
  }
}

async function installSidecarDeps() {
  console.log('Installing sidecar dependencies...');
  try {
    execSync('npm install --production', {
      cwd: sidecarDir,
      stdio: 'inherit',
      env: { ...process.env } // Ensure path and other env vars are passed
    });
    console.log('Sidecar dependencies installed.');
  } catch (e) {
    console.error('Failed to install sidecar dependencies:', e);
    process.exit(1);
  }
}

async function main() {
  await setupNode();
  await installSidecarDeps();
}

main().catch(err => {
  console.error(err);
  process.exit(1);
});
