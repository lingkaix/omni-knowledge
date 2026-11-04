import {
  isKnowledgeVersionConflict,
  type KnowledgeClient,
  type KnowledgeNode,
} from "@openkit/knowledge-ui";

interface NshBridge {
  postMessage: (data: string) => void;
}

declare global {
  interface Window {
    nsh?: NshBridge;
    __spikeOnMessage?: (data: string) => void;
    __spikeQueue?: string[];
    __setParent?: (parentId: string | null) => void;
  }
}

interface Pending {
  resolve: (value: unknown) => void;
  reject: (error: unknown) => void;
}

/**
 * Page side of the WebView string channel.
 *
 * NSH delivers `post_message` text through `window` `"message"` events and
 * accepts page text through `window.nsh.postMessage`. Typed results are JSON
 * objects inside that string. A `version-conflict` error is the object
 * `{kind, currentVersion}` so `isKnowledgeVersionConflict` can read it.
 */
export function installBridge(): KnowledgeClient {
  const pending = new Map<number, Pending>();
  let nextId = 1;

  function post(value: unknown) {
    const bridge = window.nsh;
    if (!bridge) throw new Error("window.nsh.postMessage is missing");
    bridge.postMessage(JSON.stringify(value));
  }

  function onRaw(raw: string) {
    let message: {
      kind?: string;
      id?: number;
      ok?: unknown;
      error?: unknown;
      op?: string;
      parentId?: string | null;
    };
    try {
      message = JSON.parse(raw) as typeof message;
    } catch {
      return;
    }
    if (message.kind === "reply" && typeof message.id === "number") {
      const waiter = pending.get(message.id);
      if (!waiter) return;
      pending.delete(message.id);
      if (message.error !== undefined) waiter.reject(message.error);
      else waiter.resolve(message.ok);
      return;
    }
    if (message.kind === "host" && message.op === "set-parent") {
      window.__setParent?.(message.parentId ?? null);
      post({ kind: "ack", op: "ack-host", echo: message });
      return;
    }
    if (message.kind === "host") {
      post({ kind: "ack", op: "ack-host", echo: message });
    }
  }

  window.__spikeOnMessage = onRaw;
  for (const queued of window.__spikeQueue ?? []) onRaw(queued);
  window.__spikeQueue = [];

  function call(op: string, fields: Record<string, unknown>): Promise<unknown> {
    const id = nextId;
    nextId += 1;
    return new Promise((resolve, reject) => {
      const timer = window.setTimeout(() => {
        pending.delete(id);
        reject({ kind: "bridge-timeout", detail: op });
      }, 4000);
      pending.set(id, {
        resolve: (value) => {
          window.clearTimeout(timer);
          resolve(value);
        },
        reject: (error) => {
          window.clearTimeout(timer);
          reject(error);
        },
      });
      // `id` is the correlation number. A node id travels as `nodeId`.
      // One object cannot use `id` for both, or the reply never matches.
      post({ kind: "call", op, ...fields, id });
    });
  }

  return {
    list(parentId, cursor) {
      return call("list", { parentId, cursor: cursor ?? null }) as Promise<{
        nodes: KnowledgeNode[];
        nextCursor: string | null;
      }>;
    },
    read(id) {
      return call("read", { nodeId: id }) as ReturnType<KnowledgeClient["read"]>;
    },
    search(query, cursor) {
      return call("search", { query, cursor: cursor ?? null }) as ReturnType<
        KnowledgeClient["search"]
      >;
    },
    saveText(id, body, expectedVersion) {
      return call("saveText", { nodeId: id, body, expectedVersion }) as ReturnType<
        KnowledgeClient["saveText"]
      >;
    },
    async importFile(parentId, file) {
      const node = await call("importFile", {
        parentId,
        name: file.name,
        mediaType: file.type || "application/octet-stream",
        byteSize: file.size,
      });
      return node as KnowledgeNode;
    },
  };
}

/** True when the bridge rejected with the shared version-conflict object. */
export function conflictVersion(error: unknown): string | null {
  return isKnowledgeVersionConflict(error) ? error.currentVersion : null;
}

/** Resolves when the native shell has installed `window.nsh`. */
export function waitForNativeBridge(): Promise<void> {
  return new Promise((resolve, reject) => {
    const started = Date.now();
    const tick = () => {
      if (typeof window.nsh?.postMessage === "function") {
        resolve();
        return;
      }
      if (Date.now() - started > 8000) {
        reject(new Error("window.nsh.postMessage was not installed"));
        return;
      }
      window.setTimeout(tick, 40);
    };
    tick();
  });
}
