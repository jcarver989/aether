import type { z } from "zod";
import type { SdkMcpToolDefinition } from "./types.js";

export function tool<Schema extends z.ZodRawShape>(
  definition: SdkMcpToolDefinition<Schema>,
): SdkMcpToolDefinition<Schema> {
  return definition;
}
