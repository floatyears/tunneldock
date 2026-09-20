import { EnvCheckItem, McpMode } from "../types";

const FULL_MCP_ONLY_ENV_IDS = new Set(["node", "npm", "pi", "chappie"]);

export function getEnvironmentItemsForMode(
  items: EnvCheckItem[],
  mode: McpMode
): EnvCheckItem[] {
  if (mode === "full") return items;
  return items.filter((item) => !FULL_MCP_ONLY_ENV_IDS.has(item.id));
}
