import { describe, expect, it } from 'vitest';
import { nextFreeMdName, numberedMdName } from './inbox-names';

describe('inbox-names', () => {
	it('numbers from the second copy', () => {
		expect(numberedMdName('skill', 1)).toBe('skill.md');
		expect(numberedMdName('skill', 2)).toBe('skill-2.md');
	});

	it('skips occupied basenames including inbox-relative paths', () => {
		expect(nextFreeMdName('skill', ['MonolithInbox/skill.md'])).toBe('skill-2.md');
		expect(nextFreeMdName('skill', ['skill.md', 'skill-2.md'])).toBe('skill-3.md');
	});
});
