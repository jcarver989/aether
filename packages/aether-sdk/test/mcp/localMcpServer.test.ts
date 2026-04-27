import { once } from "node:events";
import { connect, type Socket } from "node:net";

import { afterEach, describe, expect, it } from "vitest";
import { z } from "zod";

import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StreamableHTTPClientTransport } from "@modelcontextprotocol/sdk/client/streamableHttp.js";

import { LocalMcpServerHost } from "../../src/mcp/localMcpServer.js";
import { tool } from "../../src/tool.js";

describe("LocalMcpServerHost", () => {
  let host: LocalMcpServerHost | null = null;

  afterEach(async () => {
    await host?.stop().catch(() => undefined);
    host = null;
  });

  function createHost() {
    let count = 0;
    const submit = tool({
      name: "increment",
      description: "increment a closure-backed counter",
      inputSchema: { delta: z.number() },
      handler: async ({ delta }) => {
        count += delta;
        return { content: [{ type: "text", text: `count=${count}` }] };
      },
      annotations: { title: "Increment", readOnlyHint: false },
    });
    return {
      host: new LocalMcpServerHost({ name: "custom", tools: [submit] }),
      getCount: () => count,
    };
  }

  it("listTools returns the registered tool with annotations", async () => {
    const fixture = createHost();
    host = fixture.host;
    const info = await host.start();
    const client = new Client({ name: "test", version: "1.0" });
    const transport = new StreamableHTTPClientTransport(new URL(info.url), {
      requestInit: { headers: { Authorization: `Bearer ${info.authToken}` } },
    });
    await client.connect(transport);
    try {
      const tools = await client.listTools();
      expect(tools.tools).toHaveLength(1);
      expect(tools.tools[0]?.name).toBe("increment");
      expect(tools.tools[0]?.annotations?.title).toBe("Increment");
    } finally {
      await client.close();
    }
  });

  it("callTool invokes the closure-backed handler and mutates state", async () => {
    const fixture = createHost();
    host = fixture.host;
    const info = await host.start();
    const client = new Client({ name: "test", version: "1.0" });
    const transport = new StreamableHTTPClientTransport(new URL(info.url), {
      requestInit: { headers: { Authorization: `Bearer ${info.authToken}` } },
    });
    await client.connect(transport);
    try {
      await client.callTool({ name: "increment", arguments: { delta: 3 } });
      await client.callTool({ name: "increment", arguments: { delta: 4 } });
    } finally {
      await client.close();
    }
    expect(fixture.getCount()).toBe(7);
  });

  it("rejects requests without the bearer token (401)", async () => {
    const fixture = createHost();
    host = fixture.host;
    const info = await host.start();

    const response = await fetch(info.url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/list" }),
    });
    expect(response.status).toBe(401);
  });

  it("rejects requests with the wrong bearer token (401)", async () => {
    const fixture = createHost();
    host = fixture.host;
    const info = await host.start();

    const response = await fetch(info.url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
        Authorization: "Bearer not-the-real-token",
      },
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/list" }),
    });
    expect(response.status).toBe(401);
  });

  it("rejects requests whose Host header is not allowed (DNS rebinding protection)", async () => {
    const fixture = createHost();
    host = fixture.host;
    const info = await host.start();

    const response = await fetch(info.url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
        Authorization: `Bearer ${info.authToken}`,
        Host: "evil.example.com",
      },
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/list" }),
    });
    expect(response.ok).toBe(false);
    expect(response.status).toBeGreaterThanOrEqual(400);
  });

  it("stop() force-closes active HTTP connections", async () => {
    const fixture = createHost();
    host = fixture.host;
    const info = await host.start();
    const socket = await openUnfinishedMcpRequest(info.url, info.authToken);
    const socketClosed = new Promise<void>((resolve) => {
      socket.once("close", () => resolve());
    });

    try {
      await host.stop();
      host = null;
      await socketClosed;
      expect(socket.destroyed).toBe(true);
    } finally {
      socket.destroy();
    }
  });

  it("stop() closes the listening port", async () => {
    const fixture = createHost();
    host = fixture.host;
    const info = await host.start();
    await host.stop();
    host = null;

    await expect(
      fetch(info.url, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${info.authToken}`,
        },
        body: "{}",
      }),
    ).rejects.toThrow();
  });
});

async function openUnfinishedMcpRequest(
  urlString: string,
  authToken: string,
): Promise<Socket> {
  const url = new URL(urlString);
  const socket = connect(Number(url.port), url.hostname);
  socket.on("error", () => undefined);
  await once(socket, "connect");
  socket.write(
    [
      `POST ${url.pathname} HTTP/1.1`,
      `Host: ${url.host}`,
      `Authorization: Bearer ${authToken}`,
      "Content-Type: application/json",
      "Accept: application/json",
      "Content-Length: 1000000",
      "",
      "{",
    ].join("\r\n"),
  );
  return socket;
}
