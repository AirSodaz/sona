import React from 'react';

const ALLOWED_FORMAT_TAGS = new Set(['b', 'i', 'u']);

type AllowedFormatTag = 'b' | 'i' | 'u';

type RichTextNode = string | {
    tag: AllowedFormatTag;
    children: RichTextNode[];
};

interface RichTextFrame {
    tag: AllowedFormatTag | null;
    children: RichTextNode[];
}

function parseSafeTranscriptRichText(text: string): RichTextNode[] {
    const root: RichTextNode[] = [];
    const stack: RichTextFrame[] = [{ tag: null, children: root }];
    let offset = 0;

    const appendText = (value: string) => {
        if (value) {
            stack[stack.length - 1].children.push(value);
        }
    };

    while (offset < text.length) {
        if (text[offset] === '\n') {
            appendText('\n');
            offset += 1;
            continue;
        }

        const remaining = text.slice(offset);
        const tagMatch = remaining.match(/^<\/?(b|i|u)>/i);
        if (tagMatch) {
            const rawTag = tagMatch[0];
            const tag = tagMatch[1].toLowerCase() as AllowedFormatTag;
            if (rawTag.startsWith('</')) {
                if (stack.length > 1 && stack[stack.length - 1].tag === tag) {
                    stack.pop();
                    offset += rawTag.length;
                    continue;
                }
            } else {
                const node: RichTextNode = { tag, children: [] };
                stack[stack.length - 1].children.push(node);
                stack.push(node);
                offset += rawTag.length;
                continue;
            }
        }

        const nextSpecial = findNextSpecialOffset(text, offset + 1);
        appendText(text.slice(offset, nextSpecial));
        offset = nextSpecial;
    }

    return root;
}

function findNextSpecialOffset(text: string, start: number): number {
    const nextTag = text.indexOf('<', start);
    const nextNewline = text.indexOf('\n', start);
    const candidates = [nextTag, nextNewline].filter((value) => value !== -1);
    return candidates.length > 0 ? Math.min(...candidates) : text.length;
}

function renderRichTextNodes(nodes: RichTextNode[], keyPrefix: string): React.ReactNode[] {
    return nodes.map((node, index) => {
        const key = `${keyPrefix}-${index}`;
        if (typeof node === 'string') {
            return node;
        }

        const children = renderRichTextNodes(node.children, key);
        if (node.tag === 'b') {
            return <b key={key}>{children}</b>;
        }
        if (node.tag === 'i') {
            return <i key={key}>{children}</i>;
        }
        return <u key={key}>{children}</u>;
    });
}

function escapeHtmlText(text: string): string {
    const span = document.createElement('span');
    span.textContent = text;
    return span.innerHTML;
}

function serializeRichTextNodes(nodes: RichTextNode[]): string {
    return nodes.map((node) => {
        if (typeof node === 'string') {
            return escapeHtmlText(node).replace(/\n/g, '<br>');
        }

        const children = serializeRichTextNodes(node.children);
        return `<${node.tag}>${children}</${node.tag}>`;
    }).join('');
}

function parseEditorDocument(html: string): Document {
    return new DOMParser().parseFromString(html, 'text/html');
}

export function renderSafeTranscriptRichText(text: string): React.ReactNode {
    if (!text) {
        return '(empty)';
    }

    const rendered = renderRichTextNodes(parseSafeTranscriptRichText(text), 'rich-text');
    return rendered.length > 0 ? rendered : '';
}

export function transcriptTextToEditorHtml(text: string): string {
    if (!text) {
        return '';
    }

    return serializeRichTextNodes(parseSafeTranscriptRichText(text));
}

export function editorHtmlToTranscriptText(html: string): string {
    if (!html) {
        return '';
    }

    const document = parseEditorDocument(html);
    return Array.from(document.body.childNodes)
        .map((node) => serializeTranscriptNode(node))
        .join('');
}

function serializeTranscriptNode(node: Node): string {
    if (node.nodeType === Node.TEXT_NODE) {
        return (node.textContent || '').replace(/\u00a0/g, ' ');
    }

    if (node.nodeType !== Node.ELEMENT_NODE) {
        return '';
    }

    const element = node as Element;
    const tagName = element.tagName.toLowerCase();
    if (tagName === 'br') {
        return '\n';
    }

    if (tagName === 'div' || tagName === 'p') {
        const text = Array.from(element.childNodes).map(serializeTranscriptNode).join('');
        return text ? `\n${text}` : '\n';
    }

    const children = Array.from(element.childNodes).map(serializeTranscriptNode).join('');
    if (!ALLOWED_FORMAT_TAGS.has(tagName)) {
        return children;
    }

    return `<${tagName}>${children}</${tagName}>`;
}

export function getCaretCharacterOffset(element: HTMLElement): number {
    let caretOffset = 0;
    const selection = window.getSelection();
    if (selection && selection.rangeCount > 0) {
        const range = selection.getRangeAt(0);
        const preCaretRange = range.cloneRange();
        preCaretRange.selectNodeContents(element);
        preCaretRange.setEnd(range.endContainer, range.endOffset);
        caretOffset = preCaretRange.toString().length;
    }
    return caretOffset;
}

export function getRichTextNodesLength(nodes: RichTextNode[]): number {
    let length = 0;
    for (const node of nodes) {
        if (typeof node === 'string') {
            length += node.length;
        } else {
            length += getRichTextNodesLength(node.children);
        }
    }
    return length;
}

export function splitRichTextNodes(nodes: RichTextNode[], offset: number): [RichTextNode[], RichTextNode[]] {
    const left: RichTextNode[] = [];
    const right: RichTextNode[] = [];
    let currentOffset = offset;

    for (const node of nodes) {
        if (typeof node === 'string') {
            const len = node.length;
            if (currentOffset <= 0) {
                right.push(node);
            } else if (currentOffset >= len) {
                left.push(node);
                currentOffset -= len;
            } else {
                left.push(node.slice(0, currentOffset));
                right.push(node.slice(currentOffset));
                currentOffset = 0;
            }
        } else {
            const childTextLen = getRichTextNodesLength(node.children);
            if (currentOffset <= 0) {
                right.push(node);
            } else if (currentOffset >= childTextLen) {
                left.push(node);
                currentOffset -= childTextLen;
            } else {
                const [leftChildren, rightChildren] = splitRichTextNodes(node.children, currentOffset);
                if (leftChildren.length > 0) {
                    left.push({ tag: node.tag, children: leftChildren });
                }
                if (rightChildren.length > 0) {
                    right.push({ tag: node.tag, children: rightChildren });
                }
                currentOffset = 0;
            }
        }
    }

    return [left, right];
}

export function serializeNodesToTranscriptText(nodes: RichTextNode[]): string {
    return nodes.map((node) => {
        if (typeof node === 'string') {
            return node;
        }
        const children = serializeNodesToTranscriptText(node.children);
        return `<${node.tag}>${children}</${node.tag}>`;
    }).join('');
}

export function splitTranscriptText(text: string, caretOffset: number): [string, string] {
    const nodes = parseSafeTranscriptRichText(text);
    const [leftNodes, rightNodes] = splitRichTextNodes(nodes, caretOffset);
    return [
        serializeNodesToTranscriptText(leftNodes),
        serializeNodesToTranscriptText(rightNodes)
    ];
}
