import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import Editor from "./Editor";
import { useTranslation } from "@/hooks/useTranslation";

type Category = "Prompt" | "Notes" | "Docs";
type Mode = "normal" | "expanded" | "collapsed";
type Side = "left" | "right";
type Item = {
  relative_path: string;
  name: string;
  folder: string;
  is_markdown: boolean;
  is_artifact: boolean;
  preview: string;
  modified: string;
  source?: string | null;
  description?: string | null;
};
type NoteDocument = { content: string; revision: string };
type SaveResult = { status: "saved"; revision: string } | { status: "conflict"; copy: string };
type ImportCandidate = { source_path: string; relative_folder: string };

const CATEGORIES: Category[] = ["Prompt", "Notes", "Docs"];
const label = (name: Category) => name === "Docs" ? "Artifacts" : name;
const displayFolder = (name: string) => name.replace(/^Docs(?=\/|$)/, "Artifacts");

export default function SidebarWindow() {
  const { t } = useTranslation();
  const [category, setCategory] = useState<Category>("Prompt");
  const [folder, setFolder] = useState("");
  const [query, setQuery] = useState("");
  const [rows, setRows] = useState<Item[]>([]);
  const [folders, setFolders] = useState<string[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [document, setDocument] = useState<NoteDocument | null>(null);
  const [draft, setDraft] = useState("");
  const [mode, setMode] = useState<Mode>("collapsed");
  const [side, setSide] = useState<Side>("right");
  const [status, setStatus] = useState("本地文件 · 已就绪");
  const [conflict, setConflict] = useState<string | null>(null);
  const [importCandidates, setImportCandidates] = useState<ImportCandidate[]>([]);
  const [importFolder, setImportFolder] = useState("Docs");
  const [description, setDescription] = useState("");
  const [source, setSource] = useState("");
  const [creatingProject, setCreatingProject] = useState(false);
  const [projectName, setProjectName] = useState("");
  const saving = useRef(false);
  const pendingSave = useRef<Promise<{ path: string; content: string; result: SaveResult }> | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const selectedRef = useRef<string | null>(null);
  selectedRef.current = selected;

  const refresh = useCallback(async () => {
    const [nextRows, nextFolders] = await Promise.all([
      invoke<Item[]>("sidebar_list", { category, query }),
      invoke<string[]>("sidebar_list_folders"),
    ]);
    setRows(nextRows);
    setFolders(nextFolders);
  }, [category, query]);

  useEffect(() => {
    void refresh().catch((error) => setStatus(`读取失败：${String(error)}`));
  }, [refresh]);

  useEffect(() => {
    void invoke<{ side: Side; mode: Mode }>("sidebar_get_state").then((value) => {
      setSide(value.side); setMode(value.mode);
    });
    let disposed = false;
    let stopState: (() => void) | undefined;
    void listen<{ side: Side; mode: Mode }>("sidebar-state", (event) => {
      setSide(event.payload.side); setMode(event.payload.mode);
    }).then((stop) => { if (disposed) stop(); else stopState = stop; });
    return () => { disposed = true; stopState?.(); };
  }, []);

  useEffect(() => {
    let disposed = false;
    let stopEvent: (() => void) | undefined;
    let stopDrop: (() => void) | undefined;
    void listen("sidebar-files-changed", () => {
      setStatus("外部文件已同步");
      void refresh().catch((error) => setStatus(`同步失败：${String(error)}`));
    }).then((stop) => {
      if (disposed) stop(); else { stopEvent = stop; setStatus("文件监听已就绪"); }
    }).catch((error) => setStatus(`文件监听失败：${String(error)}`));
    void getCurrentWindow().onDragDropEvent((event) => {
      if (event.payload.type === "drop" && event.payload.paths.length) {
        void changeMode("normal");
        void invoke<ImportCandidate[]>("sidebar_import_candidates", { paths: event.payload.paths })
          .then((candidates) => {
            setImportCandidates(candidates); setImportFolder("Docs");
            setDescription(""); setSource("");
          }).catch((error) => setStatus(`读取导入文件失败：${String(error)}`));
      }
    }).then((stop) => { if (disposed) stop(); else stopDrop = stop; });
    return () => { disposed = true; stopEvent?.(); stopDrop?.(); };
  }, [refresh]);

  const visible = rows.filter((row) => !folder || row.folder === folder || row.folder.startsWith(`${folder}/`));
  const active = visible.find((row) => row.relative_path === selected) ?? null;

  useEffect(() => {
    if (!active?.is_markdown || active.is_artifact) { setDocument(null); setDraft(""); setConflict(null); return; }
    let cancelled = false;
    const path = active.relative_path;
    void invoke<NoteDocument>("sidebar_read_note", { relativePath: path }).then((loaded) => {
      if (!cancelled) { setDocument(loaded); setDraft(loaded.content); setConflict(null); }
    }).catch((error) => { if (!cancelled) setStatus(`打开失败：${String(error)}`); });
    return () => { cancelled = true; };
  }, [active?.relative_path, active?.is_markdown, active?.is_artifact]);

  useEffect(() => {
    if (!active?.is_markdown || active.is_artifact || !document || draft === document.content || conflict || saving.current) return;
    const path = active.relative_path;
    const currentDraft = draft;
    const expectedRevision = document.revision;
    const timer = window.setTimeout(() => {
      saving.current = true;
      setStatus("保存中…");
      const request = invoke<SaveResult>("sidebar_save_note", {
        relativePath: path, content: currentDraft, expectedRevision,
      });
      pendingSave.current = request.then((result) => ({ path, content: currentDraft, result }));
      void pendingSave.current.catch(() => {});
      void request.then((result) => {
        if (selectedRef.current !== path) return;
        if (result.status === "conflict") {
          setConflict(result.copy);
          setStatus("发现外部修改 · 已保留两版");
        } else {
          setDocument({ content: currentDraft, revision: result.revision });
          setStatus("已保存 · .md");
          void refresh();
        }
      }).catch((error) => setStatus(`保存失败：${String(error)}`)).finally(() => {
        saving.current = false;
        pendingSave.current = null;
      });
    }, 800);
    return () => window.clearTimeout(timer);
  }, [active?.relative_path, active?.is_markdown, active?.is_artifact, document, draft, conflict, refresh]);

  async function changeMode(next: Mode) {
    await invoke("sidebar_set_mode", { mode: next });
    setMode(next);
  }

  async function flushDraft() {
    if (!selected || !document || conflict) return;
    let expectedRevision = document.revision;
    if (pendingSave.current) {
      const prior = await pendingSave.current;
      if (prior.path === selected) {
        if (prior.result.status === "saved") expectedRevision = prior.result.revision;
        if (prior.content === draft) return;
      }
    }
    if (draft === document.content) return;
    saving.current = true;
    try {
      const result = await invoke<SaveResult>("sidebar_save_note", {
        relativePath: selected, content: draft, expectedRevision,
      });
      if (result.status === "conflict") {
        setConflict(result.copy);
        setStatus("发现外部修改 · 已保留两版");
      } else {
        setDocument({ content: draft, revision: result.revision });
        setStatus("已保存 · .md");
      }
    } catch (error) {
      setStatus(`保存失败：${String(error)}`);
      throw error;
    } finally { saving.current = false; }
  }

  async function chooseCategory(next: Category, open = false) {
    try {
      await flushDraft();
      setCategory(next); setFolder(""); setSelected(null); setDocument(null);
      if (open) await changeMode("normal");
    } catch { /* Keep the draft and current category visible after save failure. */ }
  }

  async function chooseItem(path: string) {
    if (path === selected) return;
    try {
      await flushDraft();
      setDocument(null);
      setSelected(path);
    } catch { /* Keep editing the unsaved draft. */ }
  }

  async function chooseFolder(next: string) {
    try {
      await flushDraft();
      setDocument(null); setSelected(null); setFolder(next);
    } catch { /* Keep editing the unsaved draft. */ }
  }

  async function changeSide() {
    const next = side === "right" ? "left" : "right";
    await invoke("sidebar_set_side", { side: next });
    setSide(next);
  }

  async function createNote() {
    try {
      await flushDraft();
      const target = folder || category;
      const item = await invoke<Item>("sidebar_create_note", { folder: target });
      setStatus("已新建 .md");
      await refresh();
      setDocument(null);
      setSelected(item.relative_path);
    } catch (error) { setStatus(`新建失败：${String(error)}`); }
  }

  async function createProject() {
    const name = projectName.trim();
    if (!name || name.includes("/") || name === "." || name === "..") {
      setStatus("项目名称不能包含路径分隔符"); return;
    }
    try {
      await flushDraft();
      const target = `${folder || category}/${name}`;
      await invoke("sidebar_create_folder", { folder: target });
      setCreatingProject(false); setProjectName("");
      await refresh();
      setSelected(null); setFolder(target);
      setStatus("已创建项目文件夹");
    } catch (error) { setStatus(`创建项目失败：${String(error)}`); }
  }

  async function trashSelected() {
    if (!selected) return;
    try {
      await flushDraft();
      await invoke("sidebar_trash_item", { relativePath: selected });
      setSelected(null); setDocument(null); setDraft("");
      setStatus("已移到系统废纸篓 · 可在 Finder 还原");
      await refresh();
    } catch (error) { setStatus(`移到废纸篓失败：${String(error)}`); }
  }

  useEffect(() => {
    const onEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      if (creatingProject) { setCreatingProject(false); return; }
      if (importCandidates.length) { setImportCandidates([]); return; }
      if (mode !== "collapsed") void flushDraft().then(() => changeMode("collapsed")).catch(() => {});
    };
    window.addEventListener("keydown", onEscape);
    return () => window.removeEventListener("keydown", onEscape);
  }, [mode, creatingProject, importCandidates, selected, document, draft, conflict]);

  useEffect(() => {
    let disposed = false;
    let stopShortcut: (() => void) | undefined;
    void listen<string>("sidebar-shortcut", (event) => {
      const action = event.payload;
      if (action === "sidebar_search") void changeMode("normal").then(() => searchRef.current?.focus());
      if (action === "sidebar_new") void changeMode("normal").then(() => createNote());
      if (action === "sidebar_prompt") void chooseCategory("Prompt", true);
      if (action === "sidebar_notes") void chooseCategory("Notes", true);
      if (action === "sidebar_docs") void chooseCategory("Docs", true);
    }).then((stop) => { if (disposed) stop(); else stopShortcut = stop; });
    return () => { disposed = true; stopShortcut?.(); };
  }, [category, folder, refresh]);

  async function importFiles() {
    const candidate = importCandidates[0];
    if (!candidate) return;
    try {
      await invoke("sidebar_import_file", {
        sourcePath: candidate.source_path,
        folder: candidate.relative_folder ? `${importFolder}/${candidate.relative_folder}` : importFolder,
        description, sourceUrl: source.trim() || null,
      });
      setImportCandidates((previous) => previous.slice(1));
      setDescription(""); setSource("");
      setCategory("Docs");
      setFolder("");
      setStatus(importCandidates.length > 1 ? `已保存，待确认 ${importCandidates.length - 1} 个交付物` : "已保存交付物");
      await refresh();
    } catch (error) { setStatus(`导入失败：${String(error)}`); }
  }

  return <div className={`sn-root sn-${mode} sn-${side}`}>
    {mode === "collapsed" ? <nav className="sn-bookmarks" aria-label="分类书签">{CATEGORIES.map((name) => <button key={name} aria-label={`展开 ${label(name)}`} onClick={() => void chooseCategory(name, true)}><span>{label(name)}</span></button>)}</nav> : <>
      <header className="sn-header"><span className="sn-mark" /><strong>{t("sidebar.brand")}</strong><span className="sn-spacer" />
        <button title="切换左右" aria-label="切换左右" onClick={() => void changeSide()}>⇄</button>
        <button title="展开或还原" aria-label="展开或还原" onClick={() => void changeMode(mode === "expanded" ? "normal" : "expanded")}>⤢</button>
        <button title="折叠侧边栏" aria-label="折叠侧边栏" onClick={() => void flushDraft().then(() => changeMode("collapsed")).catch(() => {})}>‹</button>
      </header>
      <nav className="sn-tabs" aria-label="分类">{CATEGORIES.map((name) => <button key={name} className={category === name ? "active" : ""} onClick={() => void chooseCategory(name)}>{label(name)}</button>)}</nav>
      <label className="sn-search"><span>⌕</span><input ref={searchRef} aria-label="搜索标题与正文" placeholder="搜索标题与正文" value={query} onChange={(event) => setQuery(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter" && visible[0]) void chooseItem(visible[0].relative_path); }} /></label>
      <div className="sn-folder"><select aria-label="项目" value={folder} onChange={(event) => void chooseFolder(event.target.value)}><option value="">所有项目</option>{folders.filter((name) => name.startsWith(`${category}/`)).map((name) => <option key={name} value={name}>{name.slice(category.length + 1)}</option>)}</select><button aria-label="新建项目" title="新建项目" onClick={() => setCreatingProject(true)}>＋ 项目</button><button onClick={() => void createNote()}>＋ 快记</button></div>
      <div className="sn-content"><div className="sn-list"><div className="sn-group">{displayFolder(folder || category)} · {visible.length} 项</div>{visible.length ? visible.map((row) => <button key={row.relative_path} className={`sn-row ${selected === row.relative_path ? "active" : ""}`} onClick={() => void chooseItem(row.relative_path)} onDoubleClick={() => { if (category === "Docs" && row.is_markdown) void invoke("sidebar_open_file", { relativePath: row.relative_path }).catch((error) => setStatus(`打开失败：${String(error)}`)); }}><span><i>{row.is_markdown && !row.is_artifact ? "MD" : "FILE"}</i>{row.name}</span><small>{displayFolder(row.folder)} · {row.preview.replace(/[#\n]/g, " ").slice(0, 60)}</small></button>) : <p className="sn-empty">{t("sidebar.empty")}</p>}</div>
        <main className="sn-detail">{active ? <><div className="sn-detail-head"><strong>{active.name}</strong><small>{active.is_markdown && !active.is_artifact ? "Markdown" : "原文件"}</small><button aria-label="移到废纸篓" title="移到废纸篓" onMouseDown={(event) => event.preventDefault()} onClick={() => void trashSelected()}>⌫</button></div>{active.is_markdown && !active.is_artifact ? <><div className="sn-editor">{document && <Editor key={active.relative_path} initialContent={draft} onChange={setDraft} />}</div>{conflict && <div className="sn-conflict"><p>外部编辑与草稿冲突。两版都已保留。</p><button onMouseDown={(event) => event.preventDefault()} onClick={() => { setSelected(conflict); setConflict(null); }}>编辑草稿副本</button><button onMouseDown={(event) => event.preventDefault()} onClick={() => { setSelected(null); setConflict(null); }}>查看磁盘版</button></div>}</> : <div className="sn-artifact"><h2>{active.name}</h2><p>{displayFolder(active.folder)}</p>{active.description && <p>{active.description}</p>}<button onClick={() => void invoke("sidebar_open_file", { relativePath: active.relative_path })}>打开文件</button><button onClick={() => void invoke("sidebar_show_in_finder", { relativePath: active.relative_path })}>{t("sidebar.reveal")}</button><button onClick={() => void invoke("sidebar_open_file", { relativePath: `${active.relative_path}.md` })}>编辑说明</button><p>来源：{active.source || "待确认"}</p>{active.source && <button onClick={() => void invoke("sidebar_open_source", { relativePath: active.relative_path })}>打开来源聊天</button>}</div>}</> : <div className="sn-empty sn-detail-empty">选择一条便签或交付物。</div>}</main>
      </div>
      <footer className="sn-footer"><span className="sn-dot">●</span><span>{status}</span><span className="sn-spacer" /><span>{t("sidebar.library")}</span></footer>
    </>}
    {importCandidates.length > 0 && <div className="sn-modal-backdrop"><div className="sn-modal" role="dialog" aria-modal="true" aria-label="保存交付物"><h2>保存交付物</h2><p>{importCandidates[0].source_path.split("/").pop()} · 待处理 {importCandidates.length} 项</p><label>项目<select value={importFolder} onChange={(event) => setImportFolder(event.target.value)}><option value="Docs">{t("sidebar.artifacts")}</option>{folders.filter((name) => name.startsWith("Docs/")).map((name) => <option key={name} value={name}>{displayFolder(name)}</option>)}</select></label><label>说明便签<textarea value={description} onChange={(event) => setDescription(event.target.value)} placeholder="这个交付物是什么？下次怎么找到它？" /></label><label>来源确认<input value={source} onChange={(event) => setSource(event.target.value)} placeholder="可选：Codex / 豆包工作聊天链接" /></label><p>来源由你确认；留空则标记为待确认。</p><div className="sn-modal-actions"><button onClick={() => setImportCandidates([])}>取消</button><button className="primary" onClick={() => void importFiles()}>{t("sidebar.saveArtifacts")}</button></div></div></div>}
    {creatingProject && <div className="sn-modal-backdrop"><div className="sn-modal" role="dialog" aria-modal="true" aria-label="新建项目"><h2>新建项目</h2><label>项目名称<input aria-label="项目名称" value={projectName} onChange={(event) => setProjectName(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter") void createProject(); }} /></label><div className="sn-modal-actions"><button onClick={() => setCreatingProject(false)}>取消</button><button className="primary" onClick={() => void createProject()}>创建项目</button></div></div></div>}
  </div>;
}
