import { useEffect, useState } from "react";
import { Bot, Circle, FolderOpen, Globe, LoaderCircle, Play, Plus, RefreshCw, ScanSearch, Settings as Cog, SlidersHorizontal, Square, Trash2, Braces, KeyRound } from "lucide-react";
import { useStore } from "./store";
import { StepList } from "./StepTree";
import { AddMenu } from "./AddMenu";
import { Inspector } from "./Inspector";
import { AgentPanel, ConfirmDialog, DomPanel, LogPanel, SettingsModal, VarsPanel } from "./Panels";
import { countSteps, insertAfter } from "./steps";
import { CredPromptDialog, CredsPanel } from "./Creds";

function Header({ onSettings }: { onSettings: () => void }) {
  const browser = useStore((s) => s.browser);
  const busy = useStore((s) => s.browserBusy);
  const launch = useStore((s) => s.launchBrowser);
  const call = useStore((s) => s.call);
  const models = useStore((s) => s.models);
  const ollamaError = useStore((s) => s.ollamaError);
  const settings = useStore((s) => s.settings);
  const save = useStore((s) => s.saveSettings);
  const refreshModels = useStore((s) => s.refreshModels);
  const active = browser.pages.find((p) => p.active);
  return (
    <div className="flex h-11 shrink-0 items-center gap-3 border-b border-slate-800 bg-slate-900 px-3">
      <div className="flex items-center gap-1.5 font-semibold text-slate-100">
        <Bot className="h-5 w-5 text-sky-400" /> Johnny Mecro
      </div>
      <div className="h-5 w-px bg-slate-700" />
      {browser.connected ? (
        <div className="flex min-w-0 items-center gap-2">
          <span className="h-2 w-2 shrink-0 rounded-full bg-emerald-400" title={`CDP 포트 ${browser.port}`} />
          <select
            className="inp !w-72 truncate"
            title="자동화 대상 탭"
            value={active?.targetId ?? ""}
            onChange={(e) => void call("browser_activate", { targetId: e.target.value })}
          >
            {browser.pages.length === 0 && <option value="">(열린 탭 없음)</option>}
            {browser.pages.map((p) => (
              <option key={p.targetId} value={p.targetId}>
                {(p.title || p.url || "새 탭").slice(0, 60)}
              </option>
            ))}
          </select>
          <button className="btn" title="연결 해제 (크롬은 닫히지 않습니다)" onClick={() => void call("browser_disconnect")}>
            해제
          </button>
        </div>
      ) : (
        <button className="btn btn-primary" disabled={busy} onClick={() => void launch()}>
          {busy ? <LoaderCircle className="h-4 w-4 animate-spin" /> : <Globe className="h-4 w-4" />} 크롬 실행 / 연결
        </button>
      )}
      <div className="flex-1" />
      {models.length > 0 && settings ? (
        <div className="flex items-center gap-2">
          <span className="h-2 w-2 rounded-full bg-emerald-400" title="Ollama 연결됨" />
          <span className="text-[11px] text-slate-400">추론</span>
          <select className="inp !w-44" value={settings.textModel} onChange={(e) => void save({ textModel: e.target.value })}>
            {models.map((m) => (
              <option key={m.name} value={m.name}>
                {m.name}
              </option>
            ))}
          </select>
          <span className="text-[11px] text-slate-400">비전</span>
          <select className="inp !w-44" value={settings.visionModel} onChange={(e) => void save({ visionModel: e.target.value })}>
            <option value="">(없음)</option>
            {models.map((m) => (
              <option key={m.name} value={m.name}>
                {m.name}
                {m.vision ? " 👁" : ""}
              </option>
            ))}
          </select>
        </div>
      ) : (
        <div className="flex items-center gap-2 text-amber-300" title={ollamaError}>
          <span className="h-2 w-2 rounded-full bg-amber-400" />
          {ollamaError ? "Ollama 미연결 (매크로 녹화/실행은 가능)" : "설치된 Ollama 모델 없음"}
        </div>
      )}
      <button className="btn" title="모델 목록 새로고침" onClick={() => void refreshModels()}>
        <RefreshCw className="h-4 w-4" />
      </button>
      <button className="btn" title="설정" onClick={onSettings}>
        <Cog className="h-4 w-4" />
      </button>
    </div>
  );
}

function Sidebar() {
  const scenarios = useStore((s) => s.scenarios);
  const currentId = useStore((s) => s.current?.id);
  const open = useStore((s) => s.openScenario);
  const create = useStore((s) => s.createScenario);
  const del = useStore((s) => s.deleteScenario);
  const call = useStore((s) => s.call);
  const locked = useStore((s) => s.running || s.recording);
  const [confirmId, setConfirmId] = useState<string | null>(null);
  return (
    <div className="flex w-56 shrink-0 flex-col border-r border-slate-800 bg-slate-900/50">
      <div className="flex items-center justify-between px-3 py-2">
        <span className="text-[11px] font-semibold tracking-wide text-slate-400">시나리오</span>
        <div className="flex gap-1">
          <button className="text-slate-400 hover:text-slate-100" title="시나리오 폴더 열기 (JSON 파일)" onClick={() => void call("open_scenarios_dir")}>
            <FolderOpen className="h-4 w-4" />
          </button>
          <button className="text-slate-400 hover:text-slate-100 disabled:opacity-40" title="새 시나리오" disabled={locked} onClick={() => void create()}>
            <Plus className="h-4 w-4" />
          </button>
        </div>
      </div>
      <div className="min-h-0 flex-1 overflow-auto px-1.5 pb-2">
        {scenarios.length === 0 && <div className="px-2 py-3 text-slate-600">시나리오가 없습니다. + 를 눌러 만드세요.</div>}
        {scenarios.map((s) => (
          <div
            key={s.id}
            onClick={() => void open(s.id)}
            className={`group mb-0.5 cursor-pointer rounded px-2 py-1.5 ${s.id === currentId ? "bg-sky-900/50 text-slate-50" : "text-slate-300 hover:bg-slate-800"}`}
          >
            <div className="flex items-center gap-1">
              <span className="min-w-0 flex-1 truncate">{s.name || "(이름 없음)"}</span>
              {confirmId === s.id ? (
                <button
                  className="shrink-0 rounded bg-rose-600 px-1.5 text-[11px] text-white"
                  onClick={(e) => {
                    e.stopPropagation();
                    setConfirmId(null);
                    void del(s.id);
                  }}
                  onMouseLeave={() => setConfirmId(null)}
                >
                  삭제?
                </button>
              ) : (
                !locked && (
                  <button
                    className="hidden shrink-0 text-slate-500 hover:text-rose-400 group-hover:block"
                    onClick={(e) => {
                      e.stopPropagation();
                      setConfirmId(s.id);
                    }}
                  >
                    <Trash2 className="h-3.5 w-3.5" />
                  </button>
                )
              )}
            </div>
            <div className="text-[11px] text-slate-500">
              {s.stepCount}개 스텝 · {s.updatedAt.replace("T", " ").slice(5, 16)}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function Editor() {
  const current = useStore((s) => s.current);
  const patch = useStore((s) => s.patchScenario);
  const setSteps = useStore((s) => s.setSteps);
  const select = useStore((s) => s.select);
  const selectedId = useStore((s) => s.selectedId);
  const recording = useStore((s) => s.recording);
  const running = useStore((s) => s.running);
  const runKind = useStore((s) => s.runKind);
  const startRec = useStore((s) => s.startRecording);
  const stopRec = useStore((s) => s.stopRecording);
  const run = useStore((s) => s.run);
  const stop = useStore((s) => s.stop);
  const create = useStore((s) => s.createScenario);

  if (!current)
    return (
      <div className="flex min-w-0 flex-1 flex-col items-center justify-center gap-3 text-slate-500">
        <div>시나리오를 만들고 크롬에서의 조작을 녹화하거나, 스텝을 직접 추가하세요.</div>
        <button className="btn btn-primary" onClick={() => void create()}>
          <Plus className="h-4 w-4" /> 새 시나리오
        </button>
      </div>
    );
  const locked = recording || running;
  return (
    <div className="flex min-w-0 flex-1 flex-col">
      <div className="flex shrink-0 items-center gap-2 border-b border-slate-800 px-3 py-2">
        <input
          className="inp !w-64 font-semibold"
          value={current.name}
          disabled={locked}
          onChange={(e) => patch({ name: e.target.value })}
          placeholder="시나리오 이름"
        />
        {recording ? (
          <button className="btn btn-danger" onClick={() => void stopRec()}>
            <Square className="h-4 w-4" /> 녹화 종료
          </button>
        ) : (
          <button className="btn" disabled={running} onClick={() => void startRec()} title="크롬에서의 클릭·입력·키·스크롤·이동을 스텝으로 기록합니다">
            <Circle className="h-4 w-4 fill-rose-500 text-rose-500" /> 녹화
          </button>
        )}
        {running && runKind === "scenario" ? (
          <button className="btn btn-danger" onClick={() => void stop()}>
            <Square className="h-4 w-4" /> 중지
          </button>
        ) : (
          <button className="btn btn-primary" disabled={locked || current.steps.length === 0} onClick={() => void run()}>
            <Play className="h-4 w-4" /> 실행
          </button>
        )}
        <AddMenu
          disabled={locked}
          onAdd={(step) => {
            setSteps((steps) => (selectedId ? insertAfter(steps, selectedId, step) : [...steps, step]));
            select(step.id);
          }}
        />
        <div className="flex-1" />
        <span className="text-[11px] text-slate-500">{countSteps(current.steps)}개 스텝</span>
      </div>
      {recording && (
        <div className="flex shrink-0 items-center gap-2 bg-rose-950/60 px-3 py-1.5 text-rose-200">
          <span className="h-2 w-2 animate-pulse rounded-full bg-rose-500" /> 녹화 중 — 크롬 창에서 평소처럼 조작하세요. 스텝이 실시간으로 추가됩니다.
        </div>
      )}
      <div className="min-h-0 flex-1 overflow-auto px-3 py-2" onClick={() => select(null)}>
        {current.steps.length === 0 && !recording && (
          <div className="mt-10 text-center text-slate-600">
            아직 스텝이 없습니다. <b className="text-slate-400">녹화</b>를 눌러 크롬 조작을 기록하거나 <b className="text-slate-400">스텝 추가</b>로 직접 구성하세요.
          </div>
        )}
        <StepList steps={current.steps} containerId="root" />
      </div>
    </div>
  );
}

const TABS = [
  { id: "props", label: "속성", icon: <SlidersHorizontal className="h-3.5 w-3.5" /> },
  { id: "agent", label: "에이전트", icon: <Bot className="h-3.5 w-3.5" /> },
  { id: "vars", label: "변수", icon: <Braces className="h-3.5 w-3.5" /> },
  { id: "creds", label: "로그인 정보", icon: <KeyRound className="h-3.5 w-3.5" /> },
  { id: "dom", label: "DOM", icon: <ScanSearch className="h-3.5 w-3.5" /> },
] as const;

export default function App() {
  const init = useStore((s) => s.init);
  const selectedId = useStore((s) => s.selectedId);
  const runKind = useStore((s) => s.runKind);
  const running = useStore((s) => s.running);
  const [tab, setTab] = useState<(typeof TABS)[number]["id"]>("props");
  const [showSettings, setShowSettings] = useState(false);
  useEffect(() => {
    void init();
  }, [init]);
  useEffect(() => {
    if (selectedId) setTab("props");
  }, [selectedId]);
  useEffect(() => {
    if (running && runKind === "agent") setTab("agent");
  }, [running, runKind]);

  return (
    <div className="flex h-full flex-col">
      <Header onSettings={() => setShowSettings(true)} />
      <div className="flex min-h-0 flex-1">
        <Sidebar />
        <div className="flex min-w-0 flex-1 flex-col">
          <div className="flex min-h-0 flex-1">
            <Editor />
            <div className="flex w-[440px] shrink-0 flex-col border-l border-slate-800 bg-slate-900/40">
              <div className="flex shrink-0 border-b border-slate-800">
                {TABS.map((t) => (
                  <button
                    key={t.id}
                    onClick={() => setTab(t.id)}
                    className={`flex items-center gap-1 whitespace-nowrap px-2.5 py-2 ${tab === t.id ? "border-b-2 border-sky-500 text-slate-50" : "text-slate-400 hover:text-slate-200"}`}
                  >
                    {t.icon}
                    {t.label}
                  </button>
                ))}
              </div>
              <div className="min-h-0 flex-1 overflow-auto">
                {tab === "props" && <Inspector />}
                {tab === "agent" && <AgentPanel />}
                {tab === "vars" && <VarsPanel />}
                {tab === "creds" && <CredsPanel />}
                {tab === "dom" && <DomPanel />}
              </div>
            </div>
          </div>
          <div className="h-44 shrink-0 border-t border-slate-800 bg-slate-950/70">
            <LogPanel />
          </div>
        </div>
      </div>
      <ConfirmDialog />
      <CredPromptDialog />
      {showSettings && <SettingsModal onClose={() => setShowSettings(false)} />}
    </div>
  );
}
