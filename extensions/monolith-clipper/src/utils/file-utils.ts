import browser from './browser-polyfill';
import { detectBrowser } from './browser-detection';
import { sanitizeChromeDownloadRelativePath } from './string-utils';

export interface SaveFileOptions {
	content: string;
	fileName: string;
	mimeType?: string;
	tabId?: number;
	onError?: (error: Error) => void;
}

export function base64EncodeUnicode(str: string): string {
	const utf8Bytes = encodeURIComponent(str).replace(/%([0-9A-F]{2})/g, 
		(match, p1) => String.fromCharCode(parseInt(p1, 16))
	);
	return btoa(utf8Bytes);
}

/** Chrome downloads relative to the user's Downloads folder. */
export const MONOLITH_INBOX_DIR = 'MonolithInbox';

function inboxRelativeFromAbs(absPath: string): string | undefined {
	const abs = absPath.replace(/\\/g, '/');
	const marker = `/${MONOLITH_INBOX_DIR}/`;
	const idx = abs.lastIndexOf(marker);
	if (idx !== -1) {
		return abs.slice(idx + 1);
	}
	const base = abs.split('/').pop();
	return base ? `${MONOLITH_INBOX_DIR}/${base}` : undefined;
}

/** Resolve final path after Chrome uniquify (may differ from requested name). */
export async function waitForDownloadInboxPath(
	downloadId: number,
	timeoutMs = 4_000,
): Promise<string | undefined> {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		const items = await browser.downloads.search({ id: downloadId });
		const item = items?.[0] as { state?: string; filename?: string } | undefined;
		if (!item) break;
		if (item.state === 'complete' && item.filename) {
			return inboxRelativeFromAbs(item.filename);
		}
		if (item.state === 'interrupted') {
			return undefined;
		}
		await new Promise((r) => setTimeout(r, 40));
	}
	return undefined;
}

/**
 * Queue a chrome.downloads write.
 * Prefer data: URLs — they work in MV3 service workers (no createObjectURL)
 * and survive popup close once download() returns an id.
 */
export async function downloadViaChromeApi(
	content: string,
	fileName: string,
	mimeType: string
): Promise<number> {
	const canBlob =
		typeof URL !== 'undefined' &&
		typeof URL.createObjectURL === 'function' &&
		content.length >= 1_200_000;
	let url: string;
	let revoke: (() => void) | null = null;
	if (canBlob) {
		const blob = new Blob([content], { type: `${mimeType};charset=utf-8` });
		url = URL.createObjectURL(blob);
		revoke = () => URL.revokeObjectURL(url);
	} else {
		url = `data:${mimeType};charset=utf-8,${encodeURIComponent(content)}`;
	}

	try {
		const safeName = sanitizeChromeDownloadRelativePath(fileName);
		const downloadId = await browser.downloads.download({
			url,
			filename: safeName,
			saveAs: false,
			conflictAction: 'uniquify'
		});
		if (revoke) {
			const onChanged = (delta: { id: number; state?: { current?: string } }) => {
				if (delta.id !== downloadId) return;
				const state = delta.state?.current;
				if (state === 'complete' || state === 'interrupted') {
					browser.downloads.onChanged.removeListener(onChanged);
					revoke?.();
				}
			};
			browser.downloads.onChanged.addListener(onChanged);
			setTimeout(() => {
				try {
					browser.downloads.onChanged.removeListener(onChanged);
				} catch {
					/* ignore */
				}
				revoke?.();
			}, 60_000);
		}
		return downloadId;
	} catch (err) {
		revoke?.();
		throw err;
	}
}

/**
 * Save under Downloads/MonolithInbox/.
 * @returns inbox-relative path actually written (after uniquify), when known.
 */
export async function saveFile({
	content,
	fileName,
	mimeType = 'text/markdown',
	tabId,
	onError
}: SaveFileOptions): Promise<string | undefined> {
	try {
		if (mimeType === 'text/markdown' && !fileName.toLowerCase().endsWith('.md')) {
			fileName = `${fileName}.md`;
		}
		if (mimeType === 'text/html' && !fileName.toLowerCase().endsWith('.html') && !fileName.toLowerCase().endsWith('.htm')) {
			fileName = `${fileName}.html`;
		}
		// Always land under Downloads/MonolithInbox/.
		if (!fileName.startsWith(`${MONOLITH_INBOX_DIR}/`)) {
			fileName = `${MONOLITH_INBOX_DIR}/${fileName.replace(/^\/+/, '')}`;
		}
		// Chrome IsSafePortableRelativePath — strip ? * : " etc. (Mac FS allows them).
		fileName = sanitizeChromeDownloadRelativePath(fileName);

		const browserType = await detectBrowser();
		const isSafari = ['ios', 'mobile-ios', 'ipad-os', 'safari', 'mobile-safari'].includes(browserType);

		// Chromium: call downloads API in the popup/page context (proven path).
		// Do not round-trip content through the SW — old builds awaited "complete"
		// before sendResponse and MV3 timed out, so clips looked broken.
		if (!isSafari && typeof browser.downloads?.download === 'function') {
			const id = await downloadViaChromeApi(content, fileName, mimeType);
			return (await waitForDownloadInboxPath(id, 8_000)) || fileName;
		}
		
		if (isSafari) {
			const blob = new Blob([content], { type: 'application/json' });
			const file = new File([blob], fileName, { type: 'application/json' });
			const dataUri = `data:${mimeType};charset=utf-8,${encodeURIComponent(content)}`;

			// Use share API if there is no tab ID, e.g. in settings pages
			if (!tabId) {				
				if (navigator.share) {
					try {
						await navigator.share({
							files: [file],
							text: fileName
						});
					} catch (error) {
						console.error('Error sharing:', error);
						// Fallback to opening in a new tab if sharing fails
						window.open(dataUri);
					}
				} else {
					// Fallback for older iOS versions
					window.open(dataUri);
				}
				throw new Error('Tab ID is required for saving files in Safari');
			}

			await browser.scripting.executeScript({
				target: { tabId },
				func: (fileName: string, dataUri: string) => {
					const a = document.createElement('a');
					a.href = dataUri;
					a.download = fileName;
					document.body.appendChild(a);
					a.click();
					document.body.removeChild(a);
				},
				args: [fileName, dataUri]
			});
			return fileName;
		} else {
			const blob = new Blob([content], { type: `${mimeType};charset=utf-8` });
			const url = URL.createObjectURL(blob);
			const a = document.createElement('a');
			a.href = url;
			a.download = fileName;
			document.body.appendChild(a);
			a.click();
			document.body.removeChild(a);
			URL.revokeObjectURL(url);
			return fileName;
		}
	} catch (error) {
		console.error('Failed to save file:', error);
		if (onError) {
			onError(error as Error);
		}
		// Must surface failure — silent undefined made the popup pretend success.
		throw error;
	}
}
