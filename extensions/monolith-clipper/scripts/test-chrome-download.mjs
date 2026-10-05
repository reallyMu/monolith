/**
 * Self-check: sanitize rules + real chrome.downloads via unpacked extension.
 * Run: node scripts/test-chrome-download.mjs
 */
import { chromium } from 'playwright';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const EXT =
	process.env.CLIPPER_EXT ||
	path.resolve(__dirname, '../../../src-tauri/resources/clipper-extension');
const INBOX = path.join(os.homedir(), 'Downloads', 'MonolithInbox');

function sanitizeChromeDownloadComponent(name) {
	let sanitized = name
		.replace(/[#|\^\[\]]/g, '')
		.replace(/[<>:"/\\?*\x00-\x1F]/g, '')
		.replace(/^\.+/, '')
		.replace(/[\s.]+$/g, '')
		.trim()
		.slice(0, 245);
	if (/^(con|prn|aux|nul|com[0-9]|lpt[0-9])(\..*)?$/i.test(sanitized)) {
		sanitized = `_${sanitized}`;
	}
	if (!sanitized) sanitized = 'untitled';
	return sanitized;
}

function sanitizeChromeDownloadRelativePath(relativePath) {
	const parts = relativePath
		.replace(/\\/g, '/')
		.split('/')
		.filter((p) => p && p !== '.' && p !== '..')
		.map(sanitizeChromeDownloadComponent);
	return parts.length ? parts.join('/') : 'untitled';
}

function assert(cond, msg) {
	if (!cond) throw new Error(msg);
}

// --- unit checks (mirrors string-utils) ---
const cases = [
	['MonolithInbox/foo?.md', 'MonolithInbox/foo.md'],
	['MonolithInbox/a*b:c|"<>.md', 'MonolithInbox/abc.md'],
	['MonolithInbox/trailing ', 'MonolithInbox/trailing'],
	['MonolithInbox/在线 0.3B?CARE.md', 'MonolithInbox/在线 0.3BCARE.md'],
	['MonolithInbox/con.md', 'MonolithInbox/_con.md'],
];
for (const [input, want] of cases) {
	const got = sanitizeChromeDownloadRelativePath(input);
	assert(got === want, `sanitize ${input} => ${got}, want ${want}`);
}
console.log('ok sanitize', cases.length, 'cases');

assert(fs.existsSync(path.join(EXT, 'manifest.json')), `extension missing: ${EXT}`);
const manifest = JSON.parse(fs.readFileSync(path.join(EXT, 'manifest.json'), 'utf8'));
console.log('extension version', manifest.version);

const stamp = `clipper-test-${Date.now()}`;
const dirtyStem = `${stamp} bad?name*:|\"<> `;
const safeRel = sanitizeChromeDownloadRelativePath(`MonolithInbox/${dirtyStem}.md`);
const safeAbs = path.join(os.homedir(), 'Downloads', ...safeRel.split('/'));
const userData = fs.mkdtempSync(path.join(os.tmpdir(), 'monolith-clipper-e2e-'));

// Branded Chrome 137+ strips --load-extension; use Playwright's Chromium.
const context = await chromium.launchPersistentContext(userData, {
	headless: false,
	args: [
		`--disable-extensions-except=${EXT}`,
		`--load-extension=${EXT}`,
		'--no-first-run',
		'--disable-default-apps',
	],
});

function findUnpackedExtensionId(dir) {
	const candidates = [
		path.join(dir, 'Default', 'Preferences'),
		path.join(dir, 'Default', 'Secure Preferences'),
	];
	for (const prefPath of candidates) {
		if (!fs.existsSync(prefPath)) continue;
		const data = JSON.parse(fs.readFileSync(prefPath, 'utf8'));
		const settings = data?.extensions?.settings || {};
		for (const [id, meta] of Object.entries(settings)) {
			const p = String(meta?.path || '');
			if (p.includes('clipper-extension') || p.includes('MonolithClipper')) {
				return id;
			}
			const name = meta?.manifest?.name || '';
			if (/monolith clipper/i.test(name)) return id;
		}
	}
	return null;
}

try {
	// Wake extension + wait until Chrome persists its id.
	const warm = await context.newPage();
	await warm.goto('https://example.com/', { waitUntil: 'domcontentloaded', timeout: 30_000 });
	let extId = null;
	for (let i = 0; i < 40; i++) {
		extId = findUnpackedExtensionId(userData);
		if (extId) break;
		const sw = context.serviceWorkers()[0];
		if (sw?.url()) {
			const m = sw.url().match(/^chrome-extension:\/\/([a-p]{32})\//);
			if (m) {
				extId = m[1];
				break;
			}
		}
		await new Promise((r) => setTimeout(r, 250));
	}
	assert(extId, 'extension id not found in test profile');
	console.log('extension id', extId);

	const page = await context.newPage();
	await page.goto(`chrome-extension://${extId}/settings.html`, {
		waitUntil: 'domcontentloaded',
		timeout: 30_000,
	});

	// Prove dirty name fails, sanitized name succeeds (chrome.downloads API).
	const dirtyResult = await page.evaluate(async (filename) => {
		try {
			await chrome.downloads.download({
				url: 'data:text/markdown;charset=utf-8,dirty',
				filename,
				saveAs: false,
			});
			return { ok: true };
		} catch (e) {
			return { ok: false, err: String(e?.message || e) };
		}
	}, `MonolithInbox/${dirtyStem}.md`);

	assert(!dirtyResult.ok, `expected dirty name to fail, got ${JSON.stringify(dirtyResult)}`);
	assert(
		/invalid filename/i.test(dirtyResult.err || ''),
		`expected Invalid filename, got ${dirtyResult.err}`,
	);
	console.log('ok dirty name rejected:', dirtyResult.err);

	const cleanResult = await page.evaluate(async ({ filename, body }) => {
		try {
			const id = await chrome.downloads.download({
				url: `data:text/markdown;charset=utf-8,${encodeURIComponent(body)}`,
				filename,
				saveAs: false,
				conflictAction: 'uniquify',
			});
			const deadline = Date.now() + 10_000;
			while (Date.now() < deadline) {
				const items = await chrome.downloads.search({ id });
				const item = items?.[0];
				if (item?.state === 'complete') {
					return { ok: true, id, path: item.filename };
				}
				if (item?.state === 'interrupted') {
					return { ok: false, err: 'interrupted' };
				}
				await new Promise((r) => setTimeout(r, 50));
			}
			return { ok: false, err: 'timeout' };
		} catch (e) {
			return { ok: false, err: String(e?.message || e) };
		}
	}, { filename: safeRel, body: `# ${stamp}\n\nok\n` });

	assert(cleanResult.ok, `sanitized download failed: ${JSON.stringify(cleanResult)}`);
	assert(
		fs.existsSync(cleanResult.path) || fs.existsSync(safeAbs),
		`file missing: ${cleanResult.path || safeAbs}`,
	);
	console.log('ok downloaded', cleanResult.path || safeAbs);

	for (const p of [cleanResult.path, safeAbs]) {
		if (p && fs.existsSync(p)) fs.unlinkSync(p);
	}
	console.log('PASS chrome download sanitize e2e');
} finally {
	await context.close();
	fs.rmSync(userData, { recursive: true, force: true });
}
