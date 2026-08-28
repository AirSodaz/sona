import { describe, expect, it } from 'vitest';
import {
    applyDownloadMirror,
    detectDownloadSource,
    downloadCandidates,
    modelscopeMirrorUrl,
} from '../mirrorCandidates';

const GITHUB_URL = 'https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx';
const HUGGINGFACE_URL = 'https://huggingface.co/ggml-org/Qwen3-ASR-0.6B-GGUF/resolve/abc/model.gguf?download=true';
const MODELSCOPE_URL = 'https://www.modelscope.cn/models/org/name/resolve/master/file.onnx';
const ALTERNATE_URL = 'https://www.modelscope.cn/models/org/alt/resolve/master/file.onnx';

describe('detectDownloadSource', () => {
    it('classifies source hosts', () => {
        expect(detectDownloadSource(GITHUB_URL)).toBe('github');
        expect(detectDownloadSource('https://raw.githubusercontent.com/org/repo/main/file')).toBe('github');
        expect(detectDownloadSource(HUGGINGFACE_URL)).toBe('huggingface');
        expect(detectDownloadSource(MODELSCOPE_URL)).toBe('modelscope');
        expect(detectDownloadSource('https://example.com/model.tar.bz2')).toBe('other');
    });

    it('rejects spoofed domains with suffix collisions', () => {
        expect(detectDownloadSource('https://evilgithubusercontent.com/file')).toBe('other');
        expect(detectDownloadSource('https://evilgithub.com/file')).toBe('other');
        expect(detectDownloadSource('https://huggingface.co.evil.com/file')).toBe('other');
        expect(detectDownloadSource('https://modelscope.cn.evil.com/file')).toBe('other');
    });
});

describe('applyDownloadMirror', () => {
    it('rewrites only supported hosts', () => {
        expect(applyDownloadMirror(GITHUB_URL, 'ghproxy')).toBe(
            `https://mirror.ghproxy.com/${GITHUB_URL}`,
        );
        expect(applyDownloadMirror(GITHUB_URL, 'ghnet')).toBe(
            `https://ghproxy.net/${GITHUB_URL}`,
        );
        expect(applyDownloadMirror(HUGGINGFACE_URL, 'hf-mirror')).toBe(
            'https://hf-mirror.com/ggml-org/Qwen3-ASR-0.6B-GGUF/resolve/abc/model.gguf?download=true',
        );
        expect(applyDownloadMirror(HUGGINGFACE_URL, 'ghproxy')).toBeNull();
        expect(applyDownloadMirror(GITHUB_URL, 'hf-mirror')).toBeNull();
        expect(applyDownloadMirror(MODELSCOPE_URL, 'auto')).toBeNull();
        expect(applyDownloadMirror(GITHUB_URL, 'direct')).toBeNull();
    });
});

describe('downloadCandidates', () => {
    it('chains direct, auto mirror, and alternate for GitHub', () => {
        expect(downloadCandidates(GITHUB_URL, 'auto', ALTERNATE_URL)).toEqual([
            GITHUB_URL,
            `https://ghproxy.net/${GITHUB_URL}`,
            ALTERNATE_URL,
        ]);
    });

    it('chains direct and hf-mirror for HuggingFace', () => {
        expect(downloadCandidates(HUGGINGFACE_URL, 'auto', null)).toEqual([
            HUGGINGFACE_URL,
            'https://hf-mirror.com/ggml-org/Qwen3-ASR-0.6B-GGUF/resolve/abc/model.gguf?download=true',
        ]);
    });

    it('keeps only the alternate for explicit direct', () => {
        expect(downloadCandidates(GITHUB_URL, 'direct', ALTERNATE_URL)).toEqual([
            GITHUB_URL,
            ALTERNATE_URL,
        ]);
    });

    it('skips mirrors that do not serve the host', () => {
        expect(downloadCandidates(HUGGINGFACE_URL, 'ghproxy', null)).toEqual([HUGGINGFACE_URL]);
    });

    it('skips the alternate for modelscope URLs and dedupes', () => {
        expect(downloadCandidates(MODELSCOPE_URL, 'auto', MODELSCOPE_URL)).toEqual([MODELSCOPE_URL]);
        expect(downloadCandidates(GITHUB_URL, 'auto', GITHUB_URL)).toEqual([
            GITHUB_URL,
            `https://ghproxy.net/${GITHUB_URL}`,
        ]);
    });
});

describe('modelscopeMirrorUrl', () => {
    it('returns null for uncurated models', () => {
        expect(modelscopeMirrorUrl('missing-model', 'model.onnx')).toBeNull();
    });
});
