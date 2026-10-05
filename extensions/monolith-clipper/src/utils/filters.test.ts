import { describe, expect, test, vi } from 'vitest';
import { applyFilters, clipperFilters } from './filters';

describe('Clipper filter adapter', () => {
	test('runs standard filter chains through Knap', () => {
		expect(applyFilters(' Shared Language ', 'trim|lower|replace:" ":"-"'))
			.toBe('shared-language');
	});

	test('adds Clipper environment filters to the registry', () => {
		expect(clipperFilters).toHaveProperty('markdown');
		expect(clipperFilters).toHaveProperty('html_to_json');
	});

	test('passes the current URL to fragment links', () => {
		const output = JSON.parse(applyFilters(
			'"Selected text"',
			'fragment_link',
			'https://example.com/article',
		));

		expect(output).toEqual([
			'Selected text [link](https://example.com/article#:~:text=Selected%20text)',
		]);
	});

	test('reports unknown filters in Clipper-only filter paths', () => {
		const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => undefined);
		try {
			expect(applyFilters('value', 'does_not_exist')).toBe('value');
			expect(errorSpy).toHaveBeenCalledWith('Invalid filter: does_not_exist');
		} finally {
			errorSpy.mockRestore();
		}
	});
});
