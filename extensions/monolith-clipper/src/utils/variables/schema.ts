import { applyFilters } from '../filters';
import { resolveSchemaVariable, valueToString } from '../resolver';

export async function processSchema(match: string, variables: { [key: string]: string }, currentUrl: string): Promise<string> {
	const [, fullSchemaKey] = match.match(/{{schema:(.*?)}}/) || [];
	if (!fullSchemaKey) {
		return '';
	}
	const [schemaKey, ...filterParts] = fullSchemaKey.split('|');
	const filtersString = filterParts.join('|');

	const value = resolveSchemaVariable(`schema:${schemaKey}`, variables);
	return applyFilters(valueToString(value), filtersString, currentUrl);
}
