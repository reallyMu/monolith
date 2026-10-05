// Template compiler for the Web Clipper template engine
// This module provides the main entry point for template compilation,
// integrating the AST-based renderer with the variable processors.

import { createEngine } from 'knap';
import { clipperFilters, type ClipperTemplateContext } from './filters';
import { processSimpleVariable } from './variables/simple';
import { processSelector, resolveSelector } from './variables/selector';
import { processSchema } from './variables/schema';
import { processPrompt } from './variables/prompt';
import { isModelVariable, processModelVariable } from './variables/model';
import { resolveSchemaVariable } from './resolver';

export interface RenderContext {
	variables: Record<string, any>;
	currentUrl: string;
	tabId?: number;
}

export type AsyncResolver = (name: string, context: RenderContext) => Promise<any>;

const engine = createEngine<ClipperTemplateContext>({ filters: clipperFilters });

interface DeferredTemplates {
	template: string;
	variables: Record<string, string>;
	expressions: DeferredExpression[];
}

interface DeferredExpression {
	token: string;
	template: string;
	kind: 'model' | 'prompt';
}

interface TemplateExpression {
	end: number;
	expression: string;
	trimLeft: boolean;
	trimRight: boolean;
}

function readTemplateExpression(text: string, start: number): TemplateExpression | null {
	let index = start + 2;
	const trimLeft = text[index] === '-';
	if (trimLeft) index++;
	const expressionStart = index;
	let quote: '"' | "'" | null = null;
	let escaped = false;

	while (index < text.length - 1) {
		const char = text[index];
		if (quote) {
			if (escaped) {
				escaped = false;
			} else if (char === '\\') {
				escaped = true;
			} else if (char === quote) {
				quote = null;
			}
			index++;
			continue;
		}

		if (char === '"' || char === "'") {
			quote = char;
			index++;
			continue;
		}

		if (char === '}' && text[index + 1] === '}') {
			const trimRight = text[index - 1] === '-';
			return {
				end: index + 2,
				expression: text.slice(expressionStart, trimRight ? index - 1 : index),
				trimLeft,
				trimRight,
			};
		}

		index++;
	}

	return null;
}

function canonicalizeDeferredExpression(expression: string): Omit<DeferredExpression, 'token'> | null {
	const value = expression.trim();
	const modelMatch = value.match(/^(modelProvider|modelId|model)(?:\s*\|\s*([\s\S]*))?$/);
	if (modelMatch) {
		const [, name, filters] = modelMatch;
		return {
			kind: 'model',
			template: `{{${name}${filters?.trim() ? `|${filters.trim()}` : ''}}}`,
		};
	}

	const hasPromptPrefix = value.startsWith('prompt:');
	const promptExpression = hasPromptPrefix ? value.slice('prompt:'.length).trimStart() : value;
	const quote = promptExpression[0];
	if (quote !== '"' && quote !== "'") return null;

	let closingQuote = -1;
	let escaped = false;
	for (let index = 1; index < promptExpression.length; index++) {
		const char = promptExpression[index];
		if (escaped) {
			escaped = false;
		} else if (char === '\\') {
			escaped = true;
		} else if (char === quote) {
			closingQuote = index;
			break;
		}
	}
	if (closingQuote === -1) return null;

	const remainder = promptExpression.slice(closingQuote + 1).trim();
	if (remainder && !remainder.startsWith('|')) return null;
	const filters = remainder ? remainder.slice(1).trim() : '';
	let prompt = promptExpression.slice(1, closingQuote);
	if (quote === "'") {
		prompt = prompt.replace(/\\'/g, "'").replace(/"/g, '\\"');
	}

	return {
		kind: 'prompt',
		template: `{{${hasPromptPrefix ? 'prompt:' : ''}"${prompt}"${filters ? `|${filters}` : ''}}}`,
	};
}

/**
 * Prompt expressions and interpreter model variables must survive the first
 * render pass. Knap remains application-neutral, so Clipper temporarily maps
 * them to ordinary variables and restores their original template syntax in
 * the rendered output for the interpreter-specific post-processing pass.
 */
function protectDeferredTemplates(text: string, variables: Record<string, any>): DeferredTemplates {
	const deferredVariables: Record<string, string> = {};
	const expressions: DeferredExpression[] = [];
	let deferredIndex = 0;
	let cursor = 0;
	let searchFrom = 0;
	let template = '';

	while (searchFrom < text.length) {
		const start = text.indexOf('{{', searchFrom);
		if (start === -1) break;
		const parsed = readTemplateExpression(text, start);
		if (!parsed) break;
		const deferred = canonicalizeDeferredExpression(parsed.expression);
		if (!deferred) {
			searchFrom = start + 2;
			continue;
		}

		let key: string;
		do {
			key = `__knap_deferred_${deferredIndex++}`;
		} while (key in variables || key in deferredVariables);
		const token = `\uE000knap-deferred-${deferredIndex}\uE001`;

		template += text.slice(cursor, start);
		template += `{{${parsed.trimLeft ? '-' : ''}${key}${parsed.trimRight ? '-' : ''}}}`;
		deferredVariables[key] = token;
		expressions.push({ ...deferred, token });
		cursor = parsed.end;
		searchFrom = parsed.end;
	}

	template += text.slice(cursor);
	return { template, variables: deferredVariables, expressions };
}

async function restoreDeferredTemplates(
	output: string,
	expressions: DeferredExpression[],
	variables: Record<string, any>,
	currentUrl: string,
): Promise<string> {
	for (const expression of expressions) {
		const replacement = expression.kind === 'prompt'
			? await processPrompt(expression.template, variables, currentUrl)
			: await processModelVariable(expression.template);
		output = output.split(expression.token).join(replacement);
	}
	return output;
}

/**
 * A function that processes a selector match string and returns the result.
 * Used to inject different selector implementations (browser vs CLI).
 */
export type SelectorProcessor = (match: string, currentUrl: string) => Promise<string>;

/**
 * Main function to compile a template with the given variables.
 *
 * @param tabId - Browser tab ID for selector resolution (0 if not applicable)
 * @param text - Template string to compile
 * @param variables - Variables available in the template
 * @param currentUrl - Current page URL for filter processing
 * @param customAsyncResolver - Optional async resolver override (defaults to browser selector resolver)
 * @param customSelectorProcessor - Optional selector processor override for post-processing
 * @returns Compiled template string
 */
export async function compileTemplate(
	tabId: number,
	text: string,
	variables: { [key: string]: any },
	currentUrl: string,
	customAsyncResolver?: AsyncResolver,
	customSelectorProcessor?: SelectorProcessor
): Promise<string> {
	// Strip text fragment from URL
	currentUrl = currentUrl.replace(/#:~:text=[^&]+(&|$)/, '');
	const deferred = protectDeferredTemplates(text, variables);

	// Keep application-specific variable resolution outside the shared engine.
	const resolveVariable = async (name: string): Promise<any> => {
		if (customAsyncResolver) {
			const value = await customAsyncResolver(name, {
				variables,
				currentUrl,
				tabId,
			});
			if (value !== undefined) {
				return value;
			}
		}

		if (name.startsWith('selector:') || name.startsWith('selectorHtml:')) {
			return resolveSelector(tabId, name);
		}
		if (name.startsWith('schema:')) {
			return resolveSchemaVariable(name, variables);
		}

		return undefined;
	};

	const result = await engine.render(deferred.template, {
		variables: {
			...variables,
			...deferred.variables,
		},
		context: { tabId, currentUrl },
		resolveVariable,
	});

	// Log any errors (but don't fail - return partial output)
	if (result.errors.length > 0) {
		console.error('Template compilation errors:', result.errors.map(e => `Line ${e.line}: ${e.message}`).join('; '));
	}
	if (result.warnings.length > 0) {
		console.warn(
			'Template compilation warnings:',
			result.warnings.map(warning =>
				`Line ${warning.line}, filter ${warning.filter}: ${warning.message}`
			).join('; '),
		);
	}

	// Skip application post-processing if no prompt/model expressions were protected.
	if (deferred.expressions.length === 0) {
		return result.output;
	}

	return restoreDeferredTemplates(result.output, deferred.expressions, variables, currentUrl);
}

/**
 * Process variables and apply filters.
 * Handles special variable types: selector, schema, prompt.
 *
 * This is called after the AST-based renderer to handle any remaining
 * variable interpolations that need special processing.
 */
export async function processVariables(
	tabId: number,
	text: string,
	variables: { [key: string]: any },
	currentUrl: string,
	customSelectorProcessor?: SelectorProcessor
): Promise<string> {
	const regex = /{{([\s\S]*?)}}/g;
	let result = text;
	let match;

	while ((match = regex.exec(result)) !== null) {
		const fullMatch = match[0];
		const trimmedMatch = match[1].trim();

		let replacement: string;

		if (trimmedMatch.startsWith('selector:') || trimmedMatch.startsWith('selectorHtml:')) {
			if (customSelectorProcessor) {
				replacement = await customSelectorProcessor(fullMatch, currentUrl);
			} else {
				replacement = await processSelector(tabId, fullMatch, currentUrl);
			}
		} else if (trimmedMatch.startsWith('schema:')) {
			replacement = await processSchema(fullMatch, variables, currentUrl);
		} else if (trimmedMatch.startsWith('"') || trimmedMatch.startsWith('prompt:')) {
			replacement = await processPrompt(fullMatch, variables, currentUrl);
		} else if (isModelVariable(trimmedMatch)) {
			replacement = await processModelVariable(fullMatch);
		} else {
			replacement = await processSimpleVariable(trimmedMatch, variables, currentUrl);
		}

		result = result.substring(0, match.index) + replacement + result.substring(match.index + fullMatch.length);
		regex.lastIndex = match.index + replacement.length;
	}

	return result;
}
