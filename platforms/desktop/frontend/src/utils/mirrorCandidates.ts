/**
 * Source-host aware download-mirror resolution.
 *
 * Mirrors only rewrite URLs they can actually serve: GitHub proxies only
 * rewrite GitHub URLs, `hf-mirror.com` only rewrites HuggingFace URLs.
 * `downloadCandidates` builds the ordered URL chain attempted by the
 * download loop: direct first, then the configured mirror (when applicable;
 * `auto` picks the source-appropriate one), then a curated ModelScope
 * alternate distribution as the last resort.
 *
 * Keep in sync with `adapters/model_downloads/src/mirror.rs`.
 */
import mirrorsData from '../../../../../core/src/models/model-mirrors.json';

export type DownloadSource = 'github' | 'huggingface' | 'modelscope' | 'other';

const GHPROXY_PREFIX = 'https://mirror.ghproxy.com/';
const GHNET_PREFIX = 'https://ghproxy.net/';
const HF_MIRROR_ORIGIN = 'https://hf-mirror.com';
const HUGGINGFACE_ORIGIN = 'https://huggingface.co';

type ModelMirrorsFile = {
    modelscope?: Record<string, Record<string, string>>;
};

const modelMirrors = (mirrorsData as ModelMirrorsFile).modelscope ?? {};

/** Returns the curated ModelScope alternate URL for a preset artifact. */
export function modelscopeMirrorUrl(modelId: string, filename: string): string | null {
    return modelMirrors[modelId]?.[filename] ?? null;
}

function hostOf(url: string): string {
    const withoutScheme = url.includes('://') ? url.slice(url.indexOf('://') + 3) : url;
    return (withoutScheme.split(/[/?#]/)[0] ?? '').replace(/\.$/, '').toLowerCase();
}

/** Classifies a download URL by its host family. */
export function detectDownloadSource(url: string): DownloadSource {
    const host = hostOf(url);
    if (host === 'github.com' || host.endsWith('.github.com') || host.endsWith('githubusercontent.com')) {
        return 'github';
    }
    if (host === 'huggingface.co' || host.endsWith('.huggingface.co')) {
        return 'huggingface';
    }
    if (host === 'modelscope.cn' || host.endsWith('.modelscope.cn')) {
        return 'modelscope';
    }
    return 'other';
}

/** Rewrites `url` for the given mirror, or null when the mirror does not serve its host. */
export function applyDownloadMirror(url: string, mirror: string): string | null {
    const source = detectDownloadSource(url);
    switch (mirror) {
        case 'ghproxy':
            return source === 'github' ? `${GHPROXY_PREFIX}${url}` : null;
        case 'ghnet':
            return source === 'github' ? `${GHNET_PREFIX}${url}` : null;
        case 'hf-mirror':
            return url.startsWith(HUGGINGFACE_ORIGIN)
                ? `${HF_MIRROR_ORIGIN}${url.slice(HUGGINGFACE_ORIGIN.length)}`
                : null;
        default:
            return null;
    }
}

function autoMirrorFor(source: DownloadSource): string | null {
    if (source === 'github') return 'ghnet';
    if (source === 'huggingface') return 'hf-mirror';
    return null;
}

/** Ordered download URLs to attempt for a single artifact. */
export function downloadCandidates(url: string, mirror: string, alternateUrl?: string | null): string[] {
    const candidates = [url];
    const source = detectDownloadSource(url);

    if (mirror !== 'direct') {
        const effectiveMirror = mirror === 'auto' ? autoMirrorFor(source) : mirror;
        const mirrored = effectiveMirror ? applyDownloadMirror(url, effectiveMirror) : null;
        if (mirrored && !candidates.includes(mirrored)) {
            candidates.push(mirrored);
        }
    }

    if (alternateUrl && source !== 'modelscope' && !candidates.includes(alternateUrl)) {
        candidates.push(alternateUrl);
    }

    return candidates;
}
