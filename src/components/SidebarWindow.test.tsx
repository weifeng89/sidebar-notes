import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import SidebarWindow from "./SidebarWindow";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ onDragDropEvent: async () => () => {} }),
}));
vi.mock("./Editor", () => ({
  default: ({ initialContent, onChange }: { initialContent: string; onChange: (value: string) => void }) => <textarea aria-label="Markdown 编辑器" defaultValue={initialContent} onChange={(event) => onChange(event.target.value)} />,
}));

const prompt = {
  relative_path: "Prompt/GTM/gtm.md", name: "gtm.md", folder: "Prompt/GTM",
  is_markdown: true, preview: "# GTM template", modified: "2026-10-22",
};
const artifact = {
  relative_path: "Docs/GTM/deck.pdf", name: "deck.pdf", folder: "Docs/GTM",
  is_markdown: false, preview: "", modified: "2026-10-21",
};

beforeEach(() => {
  vi.mocked(invoke).mockReset().mockImplementation(async (command, args) => {
    if (command === "sidebar_list") {
      return (args as { category: string }).category === "Prompt" ? [prompt] :
        (args as { category: string }).category === "Docs" ? [artifact] : [];
    }
    if (command === "sidebar_read_note") return { content: "# GTM template", revision: "v1" };
    if (command === "sidebar_save_note") return { status: "saved", revision: "v2" };
    if (command === "sidebar_get_state") return { side: "right", mode: "collapsed" };
    if (command === "sidebar_list_folders") return ["Prompt/GTM", "Docs/GTM"];
    return null;
  });
});
afterEach(cleanup);

describe("Sidebar Notes user journey", () => {
  it("starts as three bookmarks and opens the chosen category", async () => {
    render(<SidebarWindow />);
    await screen.findByRole("button", { name: "展开 Artifacts" });
    expect(screen.queryByRole("textbox", { name: "搜索标题与正文" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "展开 Artifacts" }));
    expect(await screen.findByText("deck.pdf")).toBeTruthy();
    expect(screen.getByRole("textbox", { name: "搜索标题与正文" })).toBeTruthy();
  });
  it("switches real categories and shows original-format artifacts", async () => {
    render(<SidebarWindow />);
    fireEvent.click(await screen.findByRole("button", { name: "展开 Prompt" }));
    expect(await screen.findByText("gtm.md")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Artifacts" }));
    expect(await screen.findByText("deck.pdf")).toBeTruthy();
    fireEvent.click(screen.getByText("deck.pdf"));
    expect(await screen.findByRole("button", { name: "打开文件" })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Finder/ })).toBeTruthy();
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("sidebar_list", expect.objectContaining({ category: "Docs" })));
  });

  it("flushes an unsaved draft before switching categories", async () => {
    render(<SidebarWindow />);
    fireEvent.click(await screen.findByRole("button", { name: "展开 Prompt" }));
    fireEvent.click(await screen.findByText("gtm.md"));
    const editor = await screen.findByRole("textbox", { name: "Markdown 编辑器" });
    fireEvent.change(editor, { target: { value: "# changed before switching" } });
    fireEvent.click(screen.getByRole("button", { name: "Artifacts" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("sidebar_save_note", expect.objectContaining({
      relativePath: "Prompt/GTM/gtm.md", content: "# changed before switching", expectedRevision: "v1",
    })));
  });

  it("waits for an in-flight save and keeps newer typing", async () => {
    let finishFirst: ((value: { status: "saved"; revision: string }) => void) | undefined;
    const defaultInvoke = vi.mocked(invoke).getMockImplementation()!;
    let saves = 0;
    vi.mocked(invoke).mockImplementation(async (command, args) => {
      if (command === "sidebar_save_note" && ++saves === 1) {
        return new Promise((resolve) => { finishFirst = resolve; });
      }
      return defaultInvoke(command, args);
    });
    render(<SidebarWindow />);
    fireEvent.click(await screen.findByRole("button", { name: "展开 Prompt" }));
    fireEvent.click(await screen.findByText("gtm.md"));
    const editor = await screen.findByRole("textbox", { name: "Markdown 编辑器" });
    vi.useFakeTimers();
    fireEvent.change(editor, { target: { value: "first draft" } });
    await vi.advanceTimersByTimeAsync(800);
    vi.useRealTimers();
    fireEvent.change(editor, { target: { value: "newer draft" } });
    fireEvent.click(screen.getByRole("button", { name: "Artifacts" }));
    finishFirst?.({ status: "saved", revision: "v2" });
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("sidebar_save_note", expect.objectContaining({
      content: "newer draft", expectedRevision: "v2",
    })));
  });
});
