import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { KnowledgeBrowser, type KnowledgeClient } from "@openkit/knowledge-ui";
import { installBridge, waitForNativeBridge } from "./bridge";

let client: KnowledgeClient | null = null;

function clientOnce(): KnowledgeClient {
  client ??= installBridge();
  return client;
}

function postProbe(value: unknown) {
  window.nsh?.postMessage(JSON.stringify({ kind: "probe", ...((value ?? {}) as object) }));
}

function textOf(selector: string): string {
  return document.querySelector(selector)?.textContent?.trim() ?? "";
}

function clickText(selector: string, text: string) {
  const found = Array.from(document.querySelectorAll(selector)).find(
    (node) => node.textContent?.trim() === text,
  );
  if (!(found instanceof HTMLElement)) throw new Error(`missing ${selector} ${text}`);
  found.click();
}

function setControl(label: string, value: string) {
  const labels = Array.from(document.querySelectorAll("label"));
  const match = labels.find((node) => node.textContent?.includes(label));
  const input = match?.parentElement?.querySelector("input") ?? match?.querySelector("input");
  if (!(input instanceof HTMLInputElement)) throw new Error(`missing input ${label}`);
  const descriptor = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value");
  descriptor?.set?.call(input, value);
  input.dispatchEvent(new Event("input", { bubbles: true }));
  input.dispatchEvent(new Event("change", { bubbles: true }));
}

function waitFor(label: string, ready: () => boolean): Promise<void> {
  return new Promise((resolve, reject) => {
    const started = Date.now();
    const tick = () => {
      if (ready()) {
        resolve();
        return;
      }
      if (Date.now() - started > 4000) {
        reject(new Error(`timed out waiting for ${label}`));
        return;
      }
      window.setTimeout(tick, 40);
    };
    tick();
  });
}

/**
 * Drive the real `KnowledgeBrowser` and report what the document showed.
 * A failed step is part of the report. It does not throw out of the page.
 */
async function exerciseBrowser(): Promise<Record<string, unknown>> {
  const report: Record<string, unknown> = {};
  try {
    await waitFor("knowledge-ui", () => document.querySelector(".knowledge-ui") !== null);
    await waitFor("note title", () => textOf(".knowledge-ui").includes("探针笔记"));
    report.knowledgeUi = true;
    report.initialTitles = Array.from(document.querySelectorAll(".knowledge-ui-title")).map(
      (node) => node.textContent?.trim() ?? "",
    );
    clickText(".knowledge-ui-title", "探针笔记");
    await waitFor("note body", () => textOf(".knowledge-ui-preview").includes("hello from the fake client"));
    report.noteBody = textOf(".knowledge-ui-preview");
    let earlier = 0;
    while (!textOf(".knowledge-ui").includes("Diagram") && earlier < 3) {
      clickText("button", "Load earlier");
      earlier += 1;
      await waitFor(`earlier ${earlier}`, () => true);
      await new Promise((resolve) => window.setTimeout(resolve, 80));
    }
    report.loadEarlierClicks = earlier;
    clickText(".knowledge-ui-title", "Diagram");
    await waitFor("image", () => document.querySelector(".knowledge-ui-preview img") !== null);
    const image = document.querySelector(".knowledge-ui-preview img");
    report.imageSrc = image instanceof HTMLImageElement ? image.src : "";
    report.imageAlt = image instanceof HTMLImageElement ? image.alt : "";
    if (image instanceof HTMLImageElement) {
      await new Promise<void>((resolve) => {
        if (image.complete) resolve();
        else {
          image.addEventListener("load", () => resolve(), { once: true });
          image.addEventListener("error", () => resolve(), { once: true });
        }
      });
      report.imageWidth = image.naturalWidth;
    }
    clickText(".knowledge-ui-title", "Spec");
    await waitFor("pdf link", () => document.querySelector(".knowledge-ui-download") !== null);
    const link = document.querySelector(".knowledge-ui-download");
    report.pdfHref = link instanceof HTMLAnchorElement ? link.href : "";
    report.previewText = textOf(".knowledge-ui-preview");
    report.previewHasBase64 = report.previewText.toString().includes("base64");
    clickText(".knowledge-ui-title", "探针笔记");
    await waitFor("edit", () => Array.from(document.querySelectorAll("button")).some((node) => node.textContent?.trim() === "Edit"));
    clickText("button", "Edit");
    await waitFor("page body", () => document.body.innerText.includes("Page body"));
    setControl("Page body", "edited");
    clickText("button", "Save");
    await waitFor("conflict", () => document.querySelector(".knowledge-ui-conflict") !== null);
    report.conflictText = textOf(".knowledge-ui-conflict");
    setControl("Search", "note");
    clickText("button", "Search pages");
    await waitFor("search hit", () => document.querySelector(".knowledge-ui-snippet") !== null);
    report.searchTitles = Array.from(document.querySelectorAll(".knowledge-ui-title")).map(
      (node) => node.textContent?.trim() ?? "",
    );
    report.searchSnippet = textOf(".knowledge-ui-snippet");
    report.ok = true;
  } catch (error) {
    report.ok = false;
    report.failure = error instanceof Error ? error.message : String(error);
  }
  return report;
}

function installImeProbe() {
  const send = (phase: string, data: string) => {
    postProbe({ event: "ime", phase, data });
  };
  document.addEventListener("compositionstart", (event) => {
    send("start", event instanceof CompositionEvent ? event.data : "");
  });
  document.addEventListener("compositionupdate", (event) => {
    send("update", event instanceof CompositionEvent ? event.data : "");
  });
  document.addEventListener("compositionend", (event) => {
    send("end", event instanceof CompositionEvent ? event.data : "");
  });
}

function App() {
  const [parentId, setParentId] = useState<string | null>("m-library");
  useEffect(() => {
    window.__setParent = (next) => setParentId(next);
  }, []);
  return <KnowledgeBrowser client={clientOnce()} parentId={parentId} />;
}

async function main() {
  installImeProbe();
  const status = document.querySelector("#spike-status");
  try {
    await waitForNativeBridge();
    if (status) status.textContent = "Native bridge ready.";
    clientOnce();
    postProbe({ event: "bridge-ready" });
    const root = document.querySelector("#root");
    if (!root) throw new Error("missing #root");
    createRoot(root).render(<App />);
    const dom = await exerciseBrowser();
    postProbe({ event: "report", dom });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (status) status.textContent = message;
    postProbe({ event: "report", dom: { ok: false, failure: message } });
  }
}

void main();
