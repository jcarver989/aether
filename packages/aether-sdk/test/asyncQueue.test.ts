import { describe, expect, it } from "vitest";

import { AsyncQueue } from "../src/asyncQueue.js";

describe("AsyncQueue", () => {
  it("yields buffered values in order", async () => {
    const q = new AsyncQueue<number>();
    q.push(1);
    q.push(2);
    q.push(3);
    q.close();
    const seen: number[] = [];
    for await (const value of q) seen.push(value);
    expect(seen).toEqual([1, 2, 3]);
  });

  it("wakes a pending reader when push happens later", async () => {
    const q = new AsyncQueue<string>();
    const iterator = q[Symbol.asyncIterator]();
    const pending = iterator.next();
    setTimeout(() => q.push("hello"), 0);
    const result = await pending;
    expect(result).toEqual({ value: "hello", done: false });
  });

  it("returns done after close with no buffered values", async () => {
    const q = new AsyncQueue<number>();
    q.close();
    const iterator = q[Symbol.asyncIterator]();
    const result = await iterator.next();
    expect(result).toEqual({ value: undefined, done: true });
  });

  it("propagates failures to pending readers", async () => {
    const q = new AsyncQueue<number>();
    const iterator = q[Symbol.asyncIterator]();
    const pending = iterator.next();
    q.fail(new Error("boom"));
    await expect(pending).rejects.toThrow("boom");
  });

  it("propagates failures to subsequent reads", async () => {
    const q = new AsyncQueue<number>();
    q.fail(new Error("boom"));
    const iterator = q[Symbol.asyncIterator]();
    await expect(iterator.next()).rejects.toThrow("boom");
  });

  it("close()ing inside the iterator stops iteration", async () => {
    const q = new AsyncQueue<number>();
    q.push(1);
    q.push(2);
    const seen: number[] = [];
    for await (const value of q) {
      seen.push(value);
      if (value === 1) q.close();
    }
    expect(seen).toEqual([1, 2]);
  });
});
