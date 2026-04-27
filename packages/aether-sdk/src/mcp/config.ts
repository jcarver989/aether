import type * as acp from "@agentclientprotocol/sdk";

import { AetherSdkError } from "../errors.js";
import { LocalMcpServerHost } from "./localMcpServer.js";
import type {
  AetherToolGroups,
  ExternalMcpServerConfig,
  SdkMcpToolDefinition,
} from "../types.js";

export interface McpSessionConfig {
  externalMcpServers?: Record<string, ExternalMcpServerConfig>;
  tools?: AetherToolGroups;
}

export interface StartedMcpServers {
  acpServers: acp.McpServer[];
  cleanup: () => Promise<void>;
}

export async function startMcpServersForSession(
  input: McpSessionConfig = {},
): Promise<StartedMcpServers> {
  const { externalMcpServers, tools } = input;
  if (isEmpty(externalMcpServers) && isEmpty(tools)) {
    return { acpServers: [], cleanup: async () => {} };
  }

  const acpServers: acp.McpServer[] = [];
  const hosts: LocalMcpServerHost[] = [];

  const cleanup = async () => {
    await Promise.allSettled(hosts.map((h) => h.stop()));
  };

  try {
    validateServerNames(externalMcpServers, tools);

    const sdkServers = await Promise.all(
      Object.entries(tools ?? {}).map(([name, definitions]) =>
        startSdkToolGroup(name, definitions, hosts),
      ),
    );
    acpServers.push(...sdkServers);

    for (const [name, config] of Object.entries(externalMcpServers ?? {})) {
      acpServers.push(convertExternalServer(name, config));
    }
  } catch (err) {
    await cleanup();
    throw err;
  }

  return { acpServers, cleanup };
}

async function startSdkToolGroup(
  name: string,
  tools: SdkMcpToolDefinition<any>[],
  hosts: LocalMcpServerHost[],
): Promise<acp.McpServer> {
  validateToolDefinitions(name, tools);
  const host = new LocalMcpServerHost({ name, tools });
  hosts.push(host);
  const { url, authToken } = await host.start();
  return {
    type: "http",
    name,
    url,
    headers: [{ name: "Authorization", value: `Bearer ${authToken}` }],
  };
}

function convertExternalServer(
  name: string,
  config: ExternalMcpServerConfig,
): acp.McpServer {
  switch (config.type) {
    case "http":
      return {
        type: "http",
        name,
        url: config.url,
        headers: toHeaders(config.headers),
      };
    case "sse":
      return {
        type: "sse",
        name,
        url: config.url,
        headers: toHeaders(config.headers),
      };
    case "stdio":
      return {
        name,
        command: config.command,
        args: config.args ?? [],
        env: Object.entries(config.env ?? {}).flatMap(([name, value]) =>
          value === undefined ? [] : [{ name, value }],
        ),
      };
  }
}

function validateServerNames(
  externalMcpServers: Record<string, ExternalMcpServerConfig> | undefined,
  tools: AetherToolGroups | undefined,
): void {
  const seen = new Set<string>();
  for (const name of Object.keys(tools ?? {})) {
    validateServerName(name, `tools.${name}`);
    addUniqueServerName(seen, name);
  }
  for (const name of Object.keys(externalMcpServers ?? {})) {
    validateServerName(name, `externalMcpServers.${name}`);
    addUniqueServerName(seen, name);
  }
}

function validateServerName(name: string, field: string): void {
  if (name.trim().length === 0) {
    throw new AetherSdkError(
      "mcp_server_invalid_config",
      `${field} must be a non-empty MCP server name`,
    );
  }
  if (name.includes("__")) {
    throw new AetherSdkError(
      "mcp_server_invalid_config",
      `${field} must not contain "__"`,
    );
  }
}

function addUniqueServerName(seen: Set<string>, name: string): void {
  if (seen.has(name)) {
    throw new AetherSdkError(
      "mcp_server_invalid_config",
      `Duplicate MCP server name "${name}"`,
    );
  }
  seen.add(name);
}

function validateToolDefinitions(
  name: string,
  tools: SdkMcpToolDefinition<any>[],
): void {
  const toolNames = new Set<string>();
  for (const definition of tools) {
    if (definition.name.trim().length === 0) {
      throw new AetherSdkError(
        "mcp_server_invalid_config",
        `tools.${name} contains a tool with an empty name`,
      );
    }
    if (toolNames.has(definition.name)) {
      throw new AetherSdkError(
        "mcp_server_invalid_config",
        `tools.${name} contains duplicate tool name "${definition.name}"`,
      );
    }
    toolNames.add(definition.name);
  }
}

function isEmpty(value: Record<string, unknown> | undefined): boolean {
  return !value || Object.keys(value).length === 0;
}

function toHeaders(
  headers: Record<string, string> | undefined,
): { name: string; value: string }[] {
  return Object.entries(headers ?? {}).map(([name, value]) => ({
    name,
    value,
  }));
}
