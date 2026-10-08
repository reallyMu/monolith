import { sanitizeFileName } from '../utils/string-utils';
import { generateFrontmatter as generateFrontmatterCore } from './shared';
import { Template, Property } from '../types/types';
import { generalSettings } from './storage-utils';
import {
	MONOLITH_INBOX_DIR,
	resolveInboxFilenameForSource,
	saveFile,
} from './file-utils';

export async function generateFrontmatter(properties: Property[]): Promise<string> {
	const typeMap: Record<string, string> = {};
	for (const pt of generalSettings.propertyTypes) {
		typeMap[pt.name] = pt.type;
	}
	return generateFrontmatterCore(properties, typeMap);
}

/** Ensure frontmatter carries the page URL for Monolith asset/log provenance. */
function ensureSourceUrlFrontmatter(fileContent: string, pageUrl: string): string {
	const url = pageUrl.trim();
	if (!url || (!url.startsWith('http://') && !url.startsWith('https://'))) {
		return fileContent;
	}
	const sourceLine = `source: "${url.replace(/"/g, '\\"')}"`;
	if (fileContent.startsWith('---\n')) {
		const end = fileContent.indexOf('\n---\n', 4);
		if (end !== -1) {
			let fm = fileContent.slice(4, end);
			const body = fileContent.slice(end + '\n---\n'.length);
			if (!/^source\s*:/m.test(fm) && !/^url\s*:/m.test(fm)) {
				fm = `${fm.trimEnd()}\n${sourceLine}\n`;
			}
			return `---\n${fm}---\n${body}`;
		}
	}
	return `---\n${sourceLine}\n---\n${fileContent}`;
}

function stemFromInboxMdPath(inboxPath: string, fallbackStem: string): string {
	const base = inboxPath
		.replace(new RegExp(`^${MONOLITH_INBOX_DIR}/`), '')
		.replace(/^\/+/, '');
	if (base.toLowerCase().endsWith('.md')) {
		return base.slice(0, -3);
	}
	return fallbackStem;
}

/**
 * Monolith shell: download Markdown (+ sidecar) into ~/Downloads/MonolithInbox/.
 * App may be offline; Monolith materializes conversion_log from frontmatter/sidecar on open.
 * Sidecar name follows the final uniquified MD basename.
 */
export async function saveToObsidian(
	fileContent: string,
	noteName: string,
	_path: string,
	_vault: string,
	_behavior: Template['behavior'],
	variables: Record<string, string> = {},
): Promise<void> {
	const formattedNoteName = sanitizeFileName(noteName) || 'untitled';
	const pageUrl = (variables['{{url}}'] || '').trim();
	const title = (variables['{{title}}'] || formattedNoteName).trim();
	const mdContent = ensureSourceUrlFrontmatter(fileContent, pageUrl);
	const target = await resolveInboxFilenameForSource(
		`${formattedNoteName}.md`,
		pageUrl.startsWith('http://') || pageUrl.startsWith('https://') ? pageUrl : undefined,
	);
	const writtenMd = await saveFile({
		content: mdContent,
		fileName: target.fileName,
		mimeType: 'text/markdown',
		sourceKey: pageUrl.startsWith('http://') || pageUrl.startsWith('https://') ? pageUrl : undefined,
		conflictAction: target.conflictAction,
	});
	if (pageUrl.startsWith('http://') || pageUrl.startsWith('https://')) {
		const stem = stemFromInboxMdPath(
			writtenMd || `${MONOLITH_INBOX_DIR}/${formattedNoteName}.md`,
			formattedNoteName,
		);
		const sidecar = JSON.stringify(
			{
				source: pageUrl,
				title,
				tool: 'monolith-clipper',
				clipped: new Date().toISOString(),
			},
			null,
			2,
		);
		await saveFile({
			content: sidecar,
			fileName: `${stem}.monolith-clip.json`,
			mimeType: 'application/json',
			conflictAction: 'overwrite',
		});
	}
}
