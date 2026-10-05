// @vitest-environment jsdom
// Tests for interpreter model variables ({{model}}, {{modelId}}, {{modelProvider}}).
// These are preserved through template compilation and filled in by the
// interpreter once a model has actually been used (issue #360).
import { describe, test, expect, beforeEach, vi } from 'vitest';
import { compileTemplate } from './template-compiler';
import { replaceModelVariables } from './interpreter';
import { generalSettings } from './storage-utils';
import { ModelConfig, Provider } from '../types/types';

const modelConfig: ModelConfig = {
	id: 'model-1',
	providerId: 'provider-1',
	providerModelId: 'claude-sonnet-5',
	name: 'Claude 5 Sonnet',
	enabled: true
};

const provider: Provider = {
	id: 'provider-1',
	name: 'Anthropic',
	baseUrl: 'https://api.anthropic.com/v1/messages',
	apiKey: 'test'
};

describe('Model variables in templates', () => {
	test('are preserved through compilation when interpreter is enabled', async () => {
		generalSettings.interpreterEnabled = true;
		const output = await compileTemplate(0, 'Summarized by {{model}} ({{modelId}}) from {{modelProvider}}', {}, 'https://example.com');
		expect(output).toBe('Summarized by {{model}} ({{modelId}}) from {{modelProvider}}');
	});

	test('preserve their filter chains through compilation', async () => {
		generalSettings.interpreterEnabled = true;
		const output = await compileTemplate(
			0,
			'{{model|lower|replace:" ":"-"}}',
			{},
			'https://example.com',
		);
		expect(output).toBe('{{model|lower|replace:" ":"-"}}');
	});

	test('preserves prompt expressions for the interpreter', async () => {
		generalSettings.interpreterEnabled = true;
		const output = await compileTemplate(
			0,
			'{{"Summarize this"|callout:("info","Summary",false)}}',
			{},
			'https://example.com',
		);
		expect(output).toBe('{{"Summarize this"|callout:("info","Summary",false)}}');
	});

	test('normalizes whitespace before interpreter post-processing', async () => {
		generalSettings.interpreterEnabled = true;
		const output = await compileTemplate(
			0,
			'{{ model }} / {{model | lower}} / {{ "Summarize this" | upper }}',
			{},
			'https://example.com',
		);
		expect(output).toBe('{{model}} / {{model|lower}} / {{"Summarize this"|upper}}');
	});

	test('normalizes single-quoted prompts instead of rendering them immediately', async () => {
		generalSettings.interpreterEnabled = true;
		const output = await compileTemplate(0, "{{'hello'|upper}}", {}, 'https://example.com');
		expect(output).toBe('{{"hello"|upper}}');
	});

	test('preserves nested template syntax inside prompt text', async () => {
		generalSettings.interpreterEnabled = true;
		const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => undefined);
		try {
			const output = await compileTemplate(
				0,
				'Head {{"Summarize {{title}} now"}} Tail',
				{},
				'https://example.com',
			);
			expect(output).toBe('Head {{"Summarize {{title}} now"}} Tail');
			expect(errorSpy).not.toHaveBeenCalled();
		} finally {
			errorSpy.mockRestore();
		}
	});

	test('are removed when interpreter is disabled', async () => {
		generalSettings.interpreterEnabled = false;
		const output = await compileTemplate(0, 'Summarized by {{model}}', {}, 'https://example.com');
		expect(output).toBe('Summarized by ');
	});
});

describe('replaceModelVariables', () => {
	beforeEach(() => {
		document.body.innerHTML = '';
	});

	function addTextarea(id: string, value: string): HTMLTextAreaElement {
		const textarea = document.createElement('textarea');
		textarea.id = id;
		textarea.value = value;
		document.body.appendChild(textarea);
		return textarea;
	}

	test('replaces model variables in input fields', () => {
		const textarea = addTextarea('note-content-field', 'Summarized by {{model}} ({{modelId}}) from {{modelProvider}}');
		replaceModelVariables(modelConfig, provider);
		expect(textarea.value).toBe('Summarized by Claude 5 Sonnet (claude-sonnet-5) from Anthropic');
	});

	test('applies filters to model variables', () => {
		const textarea = addTextarea('note-content-field', '{{model|lower|replace:" ":"-"}}');
		replaceModelVariables(modelConfig, provider);
		expect(textarea.value).toBe('claude-5-sonnet');
	});

	test('replaces model variables after compilation normalizes whitespace', async () => {
		generalSettings.interpreterEnabled = true;
		const compiled = await compileTemplate(0, '{{ model }} / {{model | lower}}', {}, 'https://example.com');
		const textarea = addTextarea('note-content-field', compiled);
		replaceModelVariables(modelConfig, provider);
		expect(textarea.value).toBe('Claude 5 Sonnet / claude 5 sonnet');
	});

	test('leaves other template syntax untouched', () => {
		const textarea = addTextarea('note-content-field', '{{model}} {{"a prompt"}} {{title}}');
		replaceModelVariables(modelConfig, provider);
		expect(textarea.value).toBe('Claude 5 Sonnet {{"a prompt"}} {{title}}');
	});
});
