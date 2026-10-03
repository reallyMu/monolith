import { format } from "sql-formatter";

/** Format SQL text; returns null if empty or formatter throws. */
export function formatSqlText(
  sql: string,
  language: "sql" | "postgresql" | "mysql" = "postgresql",
): string | null {
  const trimmed = sql.trim();
  if (!trimmed) return null;
  try {
    return format(sql, { language });
  } catch {
    return null;
  }
}
