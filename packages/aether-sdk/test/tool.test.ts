import { describe, expect, it } from "vitest";
import { z } from "zod";

import { tool } from "../src/tool.js";

describe("tool()", () => {
  it("returns the supplied name, description, schema, and handler", () => {
    const def = tool({
      name: "do_thing",
      description: "Does the thing",
      inputSchema: { value: z.string() },
      handler: async ({ value }) => ({
        content: [{ type: "text", text: value }],
      }),
    });
    expect(def.name).toBe("do_thing");
    expect(def.description).toBe("Does the thing");
    expect(def.inputSchema.value).toBeDefined();
    expect(typeof def.handler).toBe("function");
    expect(def.annotations).toBeUndefined();
  });

  it("preserves annotations when provided", () => {
    const def = tool({
      name: "annotated",
      description: "annotated tool",
      inputSchema: {},
      handler: async () => ({ content: [] }),
      annotations: { title: "Annotated", readOnlyHint: true },
    });
    expect(def.annotations).toEqual({ title: "Annotated", readOnlyHint: true });
  });

  it("supports closure-backed handlers that mutate state", async () => {
    const captured: string[] = [];
    const def = tool({
      name: "submit",
      description: "Submit",
      inputSchema: { value: z.string() },
      handler: async ({ value }) => {
        captured.push(value);
        return { content: [{ type: "text", text: "ok" }] };
      },
    });
    await def.handler({ value: "first" });
    await def.handler({ value: "second" });
    expect(captured).toEqual(["first", "second"]);
  });
});
