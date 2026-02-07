
window.__TAURI_INTERNALS__ = window.__TAURI_INTERNALS__ || {};

window.__TAURI_INTERNALS__.invoke = async (cmd, args) => {

    if (cmd === 'plugin:fs|exists') return true;
    if (cmd === 'plugin:fs|mkdir') return null;

    if (cmd === 'plugin:fs|read_text_file') {
        const items = [];
        for (let i = 0; i < 50; i++) {
            items.push({
                id: `item-${i}`,
                title: `Recording ${i} - Test Title`,
                timestamp: Date.now() - i * 86400000,
                duration: 60 + i * 10,
                audioPath: `audio-${i}.wav`,
                transcriptPath: `transcript-${i}.json`,
                previewText: `This is a preview text for recording ${i}. It should be truncated if it is too long.`,
                type: i % 3 === 0 ? 'batch' : 'recording',
                searchContent: `content ${i}`
            });
        }
        const json = JSON.stringify(items);
        const encoder = new TextEncoder();
        return Array.from(encoder.encode(json));
    }

    if (cmd === 'plugin:path|resolve_directory') return '/tmp';
    if (cmd === 'plugin:path|app_local_data_dir') return '/tmp/app_local_data';
    if (cmd === 'plugin:path|join') {
        if (args.paths) return args.paths.join('/');
        return 'joined/path';
    }

    return null;
};

window.__TAURI__ = window.__TAURI__ || {};
window.__TAURI__.window = {
    getCurrentWindow: () => ({
        listen: () => {},
        emit: () => {}
    })
};
