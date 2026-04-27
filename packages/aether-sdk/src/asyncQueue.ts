/**
 * Push-based async queue.
 *
 * Items pushed before a consumer arrives are buffered. Consumers iterating with
 * `for await` block until `push`, `close`, or `fail` is called.
 */
export class AsyncQueue<T> implements AsyncIterable<T> {
  private controller!: ReadableStreamDefaultController<T>;
  readonly stream: ReadableStream<T>;

  constructor() {
    this.stream = new ReadableStream<T>({
      start: (c) => {
        this.controller = c;
      },
    });
  }

  push(value: T): void {
    this.controller.enqueue(value);
  }

  close(): void {
    try {
      this.controller.close();
    } catch {}
  }

  fail(reason: unknown): void {
    try {
      this.controller.error(reason);
    } catch {}
  }

  [Symbol.asyncIterator](): AsyncIterator<T> {
    return this.stream.values({ preventCancel: true });
  }
}
