
// Mock Tauri Internals
window.__TAURI_INTERNALS__ = window.__TAURI_INTERNALS__ || {};
window.__TAURI__ = window.__TAURI__ || {};

// Mock Core functions
window.__TAURI_INTERNALS__.convertFileSrc = (filePath, protocol = 'asset') => {
    return `${protocol}://localhost/${filePath}`;
};

// Mock Event System
const listeners = new Map();

window.__TAURI_INTERNALS__.invoke = async (cmd, args) => {
  console.log(`[MockTauri] Invoke: ${cmd}`, args);

  if (cmd === 'check_gpu_availability') {
    return false;
  }

  if (cmd === 'download_file') {
    // Simulate download
    setTimeout(() => {
        emit('download-progress', { downloaded: 100, total: 100 });
    }, 100);
    return null;
  }

  if (cmd === 'cancel_download') {
      return null;
  }

  // Shell Plugin Mocks
  if (cmd === 'plugin:shell|spawn') {
    const pid = Math.floor(Math.random() * 10000);
    const program = args.program;
    const sidecarArgs = args.args || [];

    console.log(`[MockTauri] Spawning: ${program} with args`, sidecarArgs);

    // Identify mode
    const isStream = sidecarArgs.includes('stream');
    const isBatch = sidecarArgs.includes('batch');

    if (isStream) {
        startMockStream(pid);
    } else if (isBatch) {
        startMockBatch(pid);
    }

    return pid;
  }

  if (cmd === 'plugin:shell|kill') {
    console.log(`[MockTauri] Killed process ${args.pid}`);
    return null;
  }

  if (cmd === 'plugin:shell|write') {
      // Writing audio data
      return null;
  }

  // File System Mocks
  if (cmd === 'plugin:fs|exists') {
      // Pretend everything exists for now, or check specific paths
      if (args.path && args.path.includes('models')) return true;
      return false;
  }

  if (cmd === 'plugin:fs|mkdir') return null;

  // Path Mocks
  if (cmd === 'plugin:path|resolve_directory') {
      return '/mock/path';
  }

  if (cmd === 'plugin:path|resolve') {
      return '/mock/resource/path';
  }

  if (cmd === 'plugin:path|join') {
      return args.paths.join('/');
  }

  // Dialog Mocks
  if (cmd === 'plugin:dialog|open') {
      console.log('[MockTauri] Dialog Open called');
      return ['/path/to/test-file-1.wav', '/path/to/test-file-2.wav'];
  }

  // Default fallback
  return null;
};

// Event Emitting Helper
function emit(event, payload) {
    // Tauri v2 listeners
}

const originalInvoke = window.__TAURI_INTERNALS__.invoke;

function startMockStream(pid) {
    let counter = 0;
    const interval = setInterval(() => {
        counter++;
        const segment = {
            id: 'mock-' + counter,
            text: `Mock transcript segment ${counter}`,
            start: counter * 2,
            end: counter * 2 + 1.5,
            isFinal: false,
            tokens: [],
            timestamps: []
        };
        console.log('[MockTauri] Stream emitting:', JSON.stringify(segment));
    }, 2000);
}

function startMockBatch(pid) {
    setTimeout(() => {
        // Finish
    }, 1000);
}

// --------------------------------------------------------
// Path mocks
window.__TAURI__.path = {
    resolveResource: async (path) => '/mock/resource/' + path,
    appLocalDataDir: async () => '/mock/data',
    join: async (...args) => args.join('/'),
};

// FS Mocks
window.__TAURI__.fs = {
    exists: async () => true,
    mkdir: async () => {},
    remove: async () => {},
};
