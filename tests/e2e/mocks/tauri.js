
// Mock Tauri Internals
window.__TAURI_INTERNALS__ = window.__TAURI_INTERNALS__ || {};
window.__TAURI__ = window.__TAURI__ || {};

// Mock Event System
const listeners = new Map();

// Mock transformCallback (required for Channels in Tauri v2)
window.__TAURI_INTERNALS__.transformCallback = (callback) => {
    const id = Math.round(Math.random() * 1000000);
    window[`_${id}`] = (payload) => callback(payload);
    return id;
};

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
      if (!args.path) return false;
      // Pretend everything exists for now, or check specific paths
      if (args.path.includes('models')) return true;
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

  // Default fallback
  return null;
};

// Event Emitting Helper
function emit(event, payload) {
    // Tauri v2 listeners
    // In v2, window.__TAURI__.event might handle this, or internal IPC
    // Ideally we mimic the listener callback registration
    // But since we can't easily hook into the real 'listen' from @tauri-apps/api/event
    // (which registers a callback via invoke('plugin:event|listen')), we need to capture those listeners.
}

// We need to override the window.__TAURI__.event.listen if possible, or intercept the invoke('plugin:event|listen')
// Let's rely on intercepting 'plugin:event|listen'
const eventCallbacks = new Map();

const originalInvoke = window.__TAURI_INTERNALS__.invoke;
window.__TAURI_INTERNALS__.invoke = async (cmd, args) => {
    if (cmd === 'plugin:event|listen') {
        const eventName = args.event;
        const handlerId = args.handler; // In v2, this might be a callback ID or similar
        // Actually, v2 `listen` calls `invoke('plugin:event|listen', { event, windowLabel, handler })`
        // And the Rust side emits events back to the webview which triggers `window[`_${handler}`](payload)`.

        console.log(`[MockTauri] Registered listener for ${eventName}`);

        // We'll store this to manually trigger if needed, but it's hard to map back to the JS callback without the mapping logic.
        // EASIER STRATEGY: Mock the module imports at the build/bundler level? No, we are running built code.

        // EASIEST: Just mock `window.__TAURI__.event` if the app uses it via global (unlikely with modules).
        // Since the app uses `@tauri-apps/api/event`, it uses `invoke`.

        // Let's assume standard Tauri event pattern:
        // When event happens, `window.__TAURI_IPC__(...)` is called? No.

        // Let's implement a simplified mock stream:
        // The mock 'spawn' will just return.
        // We need to push data to the `Command` object listeners.
        // `Command` uses `plugin:shell|stdout` event.

        return 123; // Dummy event ID
    }

    // Pass through to our handler
    return originalInvoke(cmd, args);
};

// Mocking the event system for shell
// The shell plugin emits `plugin:shell|stdout` events.
// We need to trigger the frontend's listener.
// The frontend calls `listen('plugin:shell|stdout', ...)` ?
// No, `Command` creates its own channel.

// Let's look at `transcriptionService.ts`:
// `command.stdout.on('data', ...)`
// This relies on the shell plugin implementation.

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

        // We need to deliver this to the app.
        // Since we can't easily reverse-engineer the callback ID mapping of Tauri v2,
        // we might be blocked on deep integration testing of the sidecar via pure JS injection.

        // Use the globally exposed transcriptionService to simulate data ingestion
        if (window.transcriptionService && typeof window.transcriptionService.emitSegment === 'function') {
            window.transcriptionService.emitSegment(segment);
            console.log('[MockTauri] Stream emitted via transcriptionService:', segment.id);
        } else {
            console.warn('[MockTauri] transcriptionService not available or emitSegment missing');
        }

        console.log('[MockTauri] Stream emitting:', JSON.stringify(segment));

        // Try to trigger a standard Tauri event if the app listens to it.
        // But Command uses Channel.
    }, 2000);
}

function startMockBatch(pid) {
    setTimeout(() => {
        // Finish
        // We need to emit 'close' event
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
