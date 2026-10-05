// Unified variable resolution for the template engine
//
// Variables are stored with {{name}} as the key (e.g., variables["{{title}}"])
// This module provides consistent lookup across all template processing

import browser from './browser-polyfill';
import { sendExtractContent } from './variables/selector';

/**
 * Context for variable resolution, including optional tabId for selector support
 */
export interface ResolverContext {
	variables: { [key: string]: any };
	tabId?: number;
}

/**
 * Resolve a variable name to its value from the variables context (sync version).
 * Use resolveVariableAsync for selector support.
 * Handles:
 * - Simple variables: "title" → variables["{{title}}"] or variables["title"]
 * - Schema variables: "schema:@type" → variables["{{schema:@type}}"]
 * - Nested paths: "author.name" → variables.author.name
 * - Array access: "items[0]" → variables.items[0]
 * - Literals: "string", 123, true/false, null
 */
export function resolveVariable(name: string, variables: { [key: string]: any }): any {
	const trimmed = name.trim();

	// String literal (single or double quotes)
	if ((trimmed.startsWith('"') && trimmed.endsWith('"')) ||
		(trimmed.startsWith("'") && trimmed.endsWith("'"))) {
		return trimmed.slice(1, -1).replace(/\\(.)/g, '$1');
	}

	// Number literal
	if (/^-?\d+(\.\d+)?$/.test(trimmed)) {
		return parseFloat(trimmed);
	}

	// Boolean literals
	if (trimmed === 'true') return true;
	if (trimmed === 'false') return false;

	// Null/undefined literals
	if (trimmed === 'null') return null;
	if (trimmed === 'undefined') return undefined;

	// Schema variable: schema:key
	if (trimmed.startsWith('schema:')) {
		return resolveSchemaVariable(trimmed, variables);
	}

	// Simple key (no dots or brackets) - try {{name}} wrapper first
	if (!trimmed.includes('.') && !trimmed.includes('[')) {
		const wrappedValue = variables[`{{${trimmed}}}`];
		if (wrappedValue !== undefined) {
			return wrappedValue;
		}
		// Fall back to plain key (for locally set variables)
		if (variables[trimmed] !== undefined) {
			return variables[trimmed];
		}
	}

	// Nested path - try resolving from variables object
	return getNestedValue(variables, trimmed);
}

/**
 * Async version of resolveVariable that supports selector variables.
 * Use this in contexts where selectors need to be evaluated (set, for, if).
 */
export async function resolveVariableAsync(name: string, context: ResolverContext): Promise<any> {
	const trimmed = name.trim();

	// Selector variable: selector:CSS or selectorHtml:CSS
	if (trimmed.startsWith('selector:') || trimmed.startsWith('selectorHtml:')) {
		return resolveSelectorVariable(trimmed, context.tabId);
	}

	// For non-selector variables, use sync resolution
	return resolveVariable(name, context.variables);
}

/**
 * Resolve a selector variable by querying the content script
 */
async function resolveSelectorVariable(selectorExpr: string, tabId?: number): Promise<any> {
	if (!tabId) {
		console.error('Cannot resolve selector without tabId:', selectorExpr);
		return undefined;
	}

	const extractHtml = selectorExpr.startsWith('selectorHtml:');
	const prefix = extractHtml ? 'selectorHtml:' : 'selector:';
	const selectorPart = selectorExpr.slice(prefix.length);

	// Parse optional attribute: selector:CSS?attr
	const attrMatch = selectorPart.match(/^(.+?)\?(.+)$/);
	const selector = attrMatch ? attrMatch[1] : selectorPart;
	const attribute = attrMatch ? attrMatch[2] : undefined;

	try {
		const response = await sendExtractContent(tabId, selector.replace(/\\"/g, '"'), attribute, extractHtml);
		return response ? response.content : undefined;
	} catch (error) {
		console.error('Error extracting content by selector:', error, { selector, attribute, extractHtml });
		return undefined;
	}
}

/** Resolve a schema variable, including shorthand and array access. */
export function resolveSchemaVariable(name: string, variables: { [key: string]: any }): any {
	const schemaKey = name.startsWith('schema:') ? name.slice('schema:'.length) : name;
	const arrayMatch = schemaKey.match(/^(.*?)\[(\*|\d+)\](?:\.(.*))?$/);

	if (arrayMatch) {
		const [, arrayKey, indexOrStar, propertyPath] = arrayMatch;
		const rawValue = resolveSchemaKey(arrayKey, variables);
		if (rawValue === undefined) return undefined;

		const parsed = parseSchemaArray(rawValue);
		if (!Array.isArray(parsed)) return undefined;

		if (indexOrStar === '*') {
			return propertyPath
				? parsed.map(item => getNestedValue(item, propertyPath)).filter(value => value != null)
				: parsed;
		}

		const item = parsed[Number.parseInt(indexOrStar, 10)];
		if (item === undefined) return undefined;
		return propertyPath ? getNestedValue(item, propertyPath) : item;
	}

	const rawValue = resolveSchemaKey(schemaKey, variables);
	return rawValue === undefined ? undefined : parseSchemaValue(rawValue);
}

function resolveSchemaKey(schemaKey: string, variables: { [key: string]: any }): any {
	const name = `schema:${schemaKey}`;
	const wrappedValue = variables[`{{${name}}}`];
	if (wrappedValue !== undefined) return wrappedValue;
	if (variables[name] !== undefined) return variables[name];

	if (!schemaKey.includes('@')) {
		const matchingKey = Object.keys(variables).find(key =>
			key.includes('@') && key.endsWith(`:${schemaKey}}}`));
		if (matchingKey) return variables[matchingKey];
	}

	return undefined;
}

function parseSchemaArray(value: any): any {
	if (typeof value === 'string' && value.trim().match(/^(?:\d+\.|[-*•]\s)/m)) {
		return value
			.split(/(?=\d+\.|[-*•]\s)/)
			.map(item => item.replace(/^(?:\d+\.|[-*•])\s*/, '').trim())
			.filter(Boolean);
	}
	return parseSchemaValue(value);
}

function parseSchemaValue(value: any): any {
	if (typeof value === 'string' && (value.startsWith('[') || value.startsWith('{'))) {
		try {
			return JSON.parse(value);
		} catch {
			// Preserve malformed JSON-like strings as ordinary values.
		}
	}
	return value;
}

/**
 * Get a nested value from an object using dot notation and bracket notation.
 * Examples:
 * - "author.name" → obj.author.name
 * - "items[0]" → obj.items[0]
 * - "items[0].title" → obj.items[0].title
 */
export function getNestedValue(obj: any, path: string): any {
	if (!path || !obj) return undefined;

	const keys = path.split('.');
	return keys.reduce((value, key) => {
		if (value === undefined || value === null) return undefined;

		// Handle bracket notation for array access: items[0]
		if (key.includes('[') && key.includes(']')) {
			const match = key.match(/^([^\[]*)\[([^\]]+)\]/);
			if (match) {
				const [, arrayKey, indexStr] = match;
				const baseValue = arrayKey ? value[arrayKey] : value;
				if (Array.isArray(baseValue)) {
					const index = parseInt(indexStr, 10);
					return baseValue[index];
				}
				// Also handle object bracket notation: obj["key"]
				if (baseValue && typeof baseValue === 'object') {
					return baseValue[indexStr.replace(/^["']|["']$/g, '')];
				}
				return undefined;
			}
		}

		return value[key];
	}, obj);
}

/**
 * Convert any value to a string for template output.
 * - undefined/null → ''
 * - objects → JSON.stringify
 * - everything else → String()
 */
export function valueToString(value: any): string {
	if (value === undefined || value === null) {
		return '';
	}
	if (typeof value === 'object') {
		return JSON.stringify(value);
	}
	return String(value);
}
