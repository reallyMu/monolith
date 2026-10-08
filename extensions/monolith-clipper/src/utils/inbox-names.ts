/** `{stem}.md` or `{stem}-2.md`, `{stem}-3.md`, … */
export function numberedMdName(stem: string, n: number): string {
	if (n <= 1) return `${stem}.md`;
	return `${stem}-${n}.md`;
}

export function nextFreeMdName(stem: string, occupiedBasenames: Iterable<string>): string {
	const taken = new Set(
		[...occupiedBasenames].map((s) => s.replace(/^.*\//, '').toLowerCase()),
	);
	for (let n = 1; n < 10_000; n++) {
		const name = numberedMdName(stem, n);
		if (!taken.has(name.toLowerCase())) return name;
	}
	return `${stem}-${Date.now()}.md`;
}
