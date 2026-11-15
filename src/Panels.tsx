import { useEffect, useRef, useState } from "react";
import { Bot, FilePlus2, ListPlus, Play, ScanSearch, Square, Trash2, X, Image as ImageIcon, Plus } from "lucide-react";
import { useStore } from "./store";
import type { Settings } from "./types";
import { cloneStep, stepTitle } from "./steps";

// ---------- Tier 3 agent ----------
export function AgentPanel() {
  const [goal, setGoal] = useState("");
  const running = useStore((s) => s.running);
  const runKind = useStore((s) => s.runKind);
  const recording = useStore((s) => s.recording);
  const steps = useStore((s) => s.agentSteps);
  const trace = useStore((s) => s.agentTrace);
  const runAgent = useStore((s) => s.runAgent);
  const stop = useStore((s) => s.stop);
  const createScenario = useStore((s) => s.createScenario);
  const setSteps = useStore((s) => s.setSteps);
  const current = useStore((s) => s.current);
  const model = useStore((s) => s.settings?.textModel);
  const endRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    // newer Chromium returns a Promise from scrollIntoView; never return it as an effect cleanup
    void endRef.current?.scrollIntoView({ block: "nearest" });
  }, [steps.length, trace]);

  return (
    <div className="flex h-full flex-col p-3">
      <div className="lbl">목표 (Goal) — 현재 활성 탭에서 시작합니다</div>
      <textarea
        className="inp"
        rows={3}
        value={goal}
        placeholder="예: 검색창에 '기계식 키보드'를 검색하고 첫 번째 결과를 열어줘"
        onChange={(e) => setGoal(e.target.value)}
      />
      <div className="mt-2 flex items-center gap-2">
        {running && runKind === "agent" ? (
          <button className="btn btn-danger" onClick={() => void stop()}>
            <Square className="h-4 w-4" /> 중지
          </button>
        ) : (
          <button className="btn btn-primary" disabled={!goal.trim() || running || recording || !model} onClick={() => void runAgent(goal.trim())}>
            <Play className="h-4 w-4" /> 자율 실행
          </button>
        )}
        <span className="text-[11px] text-slate-500">{model ? `Planner → Navigator → Validator (${model})` : "Ollama 모델을 먼저 선택하세요"}</span>
      </div>

      <div className="selectable mt-3 min-h-0 flex-1 overflow-auto rounded border border-slate-800 bg-slate-950/60 p-2">
        {steps.length === 0 && !trace && <div className="text-slate-600">에이전트의 관찰·판단·행동 과정이 여기에 표시됩니다.</div>}
        {steps.map((s, i) => (
          <div key={i} className="mb-2 border-l-2 border-fuchsia-700/60 pl-2">
            <div className="text-slate-200">
              <span className="text-slate-500">{s.n}.</span> {s.thought}
            </div>
            <div className="text-[11px] text-slate-400">
              <span className={s.status === "COMPLETED" ? "text-emerald-400" : s.status === "FAILED" ? "text-rose-400" : s.status === "NEED_USER_HELP" ? "text-amber-400" : "text-sky-400"}>
                {s.status}
              </span>
              {" · "}
              {s.action}
              {s.index > 0 ? ` [${s.index}]` : ""}
              {s.input ? ` "${s.input}"` : ""}
            </div>
          </div>
        ))}
        {trace && (
          <div className={`mt-2 rounded border p-2 ${trace.success ? "border-emerald-700 bg-emerald-950/40" : "border-rose-800 bg-rose-950/30"}`}>
            <div className={trace.success ? "text-emerald-300" : "text-rose-300"}>
              {trace.success ? "✅ 목표 달성" : "✖ 목표 미달성"} — {trace.message}
            </div>
            {trace.success && trace.steps.length > 0 && (
              <>
                <div className="mt-2 text-[11px] text-slate-400">실행 경로 ({trace.steps.length}개 스텝) — 매크로로 변환하면 다음부터 LLM 없이 고속 실행됩니다</div>
                <ol className="mt-1 list-decimal pl-5 text-[12px] text-slate-300">
                  {trace.steps.map((s) => (
                    <li key={s.id}>{stepTitle(s)}</li>
                  ))}
                </ol>
                <div className="mt-2 flex flex-wrap gap-2">
                  <button className="btn btn-primary" disabled={running} onClick={() => void createScenario(trace.goal.slice(0, 40), trace.steps.map(cloneStep))}>
                    <FilePlus2 className="h-4 w-4" /> 새 시나리오로 저장 (Compile to Macro)
                  </button>
                  <button className="btn" disabled={running || !current} onClick={() => setSteps((st) => [...st, ...trace.steps.slice(1).map(cloneStep)])}>
                    <ListPlus className="h-4 w-4" /> 현재 시나리오에 추가
                  </button>
                </div>
              </>
            )}
          </div>
        )}
        <div ref={endRef} />
      </div>
    </div>
  );
}

// ---------- Module C inspection ----------
export function DomPanel() {
  const call = useStore((s) => s.call);
  const connected = useStore((s) => s.browser.connected);
  const [text, setText] = useState("");
  const [meta, setMeta] = useState("");
  const [image, setImage] = useState("");
  const [busy, setBusy] = useState(false);
  type D = { url: string; count: number; truncated: boolean; text: string };
  const show = (d: D) => {
    setText(d.text);
    setMeta(`${d.count}개 요소${d.truncated ? " (뷰포트 주변으로 축약)" : ""} · 약 ${Math.round(d.text.length / 3)} 토큰 · ${d.url}`);
  };
  return (
    <div className="flex h-full flex-col p-3">
      <div className="mb-2 flex gap-2">
        <button
          className="btn"
          disabled={!connected || busy}
          onClick={async () => {
            setBusy(true);
            const d = await call<D>("distill_page");
            if (d) {
              show(d);
              setImage("");
            }
            setBusy(false);
          }}
        >
          <ScanSearch className="h-4 w-4" /> DOM 증류
        </button>
        <button
          className="btn"
          disabled={!connected || busy}
          onClick={async () => {
            setBusy(true);
            const r = await call<{ image: string; distilled: D }>("som_screenshot");
            if (r) {
              show(r.distilled);
              setImage(r.image);
            }
            setBusy(false);
          }}
        >
          <ImageIcon className="h-4 w-4" /> Set-of-Mark 스크린샷
        </button>
      </div>
      <div className="mb-1 text-[11px] text-slate-500">{meta || "LLM에 전달되는 초경량 DOM 표현과 비전 폴백용 SoM 이미지를 미리 볼 수 있습니다."}</div>
      <div className="min-h-0 flex-1 overflow-auto rounded border border-slate-800 bg-slate-950/60">
        {image && <img src={`data:image/jpeg;base64,${image}`} className="w-full border-b border-slate-800" />}
        <pre className="selectable whitespace-pre-wrap break-all p-2 font-mono text-[11px] leading-4 text-slate-300">{text}</pre>
      </div>
    </div>
  );
}

// ---------- variables ----------
export function VarsPanel() {
  const current = useStore((s) => s.current);
  const patchScenario = useStore((s) => s.patchScenario);
  const vars = useStore((s) => s.vars);
  const locked = useStore((s) => s.running || s.recording);
  const [name, setName] = useState("");
  if (!current) return <div className="p-4 text-slate-500">시나리오를 선택하세요.</div>;
  const entries = Object.entries(current.variables);
  const setVar = (k: string, v: string) => patchScenario({ variables: { ...current.variables, [k]: v } });
  return (
    <div className="h-full overflow-auto p-3">
      <div className="lbl">시나리오 변수 — 스텝에서 {"{{이름}}"} 으로 바인딩</div>
      {entries.map(([k, v]) => (
        <div key={k} className="mb-1.5 flex items-center gap-1.5">
          <span className="w-28 shrink-0 truncate font-mono text-[12px] text-sky-300" title={k}>{`{{${k}}}`}</span>
          <input className="inp" disabled={locked} value={typeof v === "string" ? v : JSON.stringify(v)} onChange={(e) => setVar(k, e.target.value)} />
          <button
            className="text-slate-500 hover:text-rose-400"
            disabled={locked}
            onClick={() => {
              const next = { ...current.variables };
              delete next[k];
              patchScenario({ variables: next });
            }}
          >
            <Trash2 className="h-4 w-4" />
          </button>
        </div>
      ))}
      <div className="mt-2 flex gap-1.5">
        <input className="inp" placeholder="새 변수 이름" value={name} disabled={locked} onChange={(e) => setName(e.target.value.replace(/[^\w]/g, ""))} />
        <button
          className="btn shrink-0"
          disabled={!name || locked || name in current.variables}
          onClick={() => {
            setVar(name, "");
            setName("");
          }}
        >
          <Plus className="h-4 w-4" /> 추가
        </button>
      </div>
      <div className="lbl mt-5">마지막 실행의 런타임 변수 (추출 결과 포함)</div>
      <pre className="selectable whitespace-pre-wrap break-all rounded border border-slate-800 bg-slate-950/60 p-2 font-mono text-[11px] text-slate-300">
        {Object.keys(vars).length ? JSON.stringify(vars, null, 2) : "(없음)"}
      </pre>
    </div>
  );
}

// ---------- log ----------
export function LogPanel() {
  const logs = useStore((s) => s.logs);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [logs.length]);
  const color = { info: "text-slate-300", warn: "text-amber-300", error: "text-rose-400", success: "text-emerald-300" } as const;
  return (
    <div ref={ref} className="selectable h-full overflow-auto px-3 py-1.5 font-mono text-[12px] leading-5">
      {logs.length === 0 && <div className="text-slate-600">실행 로그가 여기에 표시됩니다.</div>}
      {logs.map((l, i) => (
        <div key={i} className={`whitespace-pre-wrap break-all ${color[l.level] ?? "text-slate-300"}`}>
          <span className="text-slate-600">{l.time}</span> {l.message}
        </div>
      ))}
    </div>
  );
}

// ---------- confirm (safety guardrail) ----------
export function ConfirmDialog() {
  const c = useStore((s) => s.confirm);
  const answer = useStore((s) => s.answerConfirm);
  if (!c) return null;
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="w-[440px] rounded-lg border border-amber-600 bg-slate-900 p-4 shadow-2xl">
        <div className="mb-2 flex items-center gap-2 text-base font-semibold text-amber-300">
          <Bot className="h-5 w-5" /> {c.title}
        </div>
        <div className="selectable mb-4 whitespace-pre-wrap text-slate-200">{c.message}</div>
        <div className="flex justify-end gap-2">
          <button className="btn" onClick={() => void answer(false)}>
            거부 / 중단
          </button>
          <button className="btn btn-primary" onClick={() => void answer(true)}>
            허용 / 계속
          </button>
        </div>
      </div>
    </div>
  );
}

// ---------- settings ----------
export function SettingsModal({ onClose }: { onClose: () => void }) {
  const settings = useStore((s) => s.settings);
  const save = useStore((s) => s.saveSettings);
  const refreshModels = useStore((s) => s.refreshModels);
  const [d, setD] = useState<Settings | null>(settings);
  if (!d) return null;
  const set = (p: Partial<Settings>) => setD({ ...d, ...p });
  const num = (k: keyof Settings, label: string, hint?: string) => (
    <label className="block">
      <span className="lbl">{label}</span>
      <input className="inp" type="number" value={d[k] as number} onChange={(e) => set({ [k]: Number(e.target.value) || 0 } as Partial<Settings>)} />
      {hint && <span className="text-[11px] text-slate-500">{hint}</span>}
    </label>
  );
  const txt = (k: keyof Settings, label: string, hint?: string, ph?: string) => (
    <label className="block">
      <span className="lbl">{label}</span>
      <input className="inp" value={d[k] as string} placeholder={ph} onChange={(e) => set({ [k]: e.target.value } as Partial<Settings>)} />
      {hint && <span className="text-[11px] text-slate-500">{hint}</span>}
    </label>
  );
  const chk = (k: keyof Settings, label: string) => (
    <label className="flex items-center gap-2">
      <input type="checkbox" className="accent-sky-500" checked={d[k] as boolean} onChange={(e) => set({ [k]: e.target.checked } as Partial<Settings>)} />
      {label}
    </label>
  );
  return (
    <div className="fixed inset-0 z-40 flex items-center justify-center bg-black/60" onMouseDown={onClose}>
      <div className="max-h-[90vh] w-[620px] overflow-auto rounded-lg border border-slate-700 bg-slate-900 p-4 shadow-2xl" onMouseDown={(e) => e.stopPropagation()}>
        <div className="mb-3 flex items-center justify-between">
          <div className="text-base font-semibold text-slate-100">설정</div>
          <button className="text-slate-400 hover:text-slate-100" onClick={onClose}>
            <X className="h-5 w-5" />
          </button>
        </div>
        <div className="mb-1 text-[12px] font-semibold text-sky-300">브라우저 (CDP)</div>
        <div className="grid grid-cols-2 gap-3">
          {num("debugPort", "원격 디버깅 포트", "--remote-debugging-port")}
          {txt("chromePath", "chrome.exe 경로", "비우면 자동 탐색", "자동")}
          <div className="col-span-2">
            {txt("userDataDir", "크롬 프로필 폴더 (--user-data-dir)", "비우면 앱 전용 프로필을 사용하며 로그인 세션이 유지됩니다. Chrome 136+는 기본 프로필 폴더에 대한 원격 디버깅을 차단합니다.", "앱 전용 프로필")}
          </div>
        </div>
        <div className="mb-1 mt-4 text-[12px] font-semibold text-sky-300">로컬 LLM (Ollama)</div>
        <div className="grid grid-cols-2 gap-3">
          <div className="col-span-2">{txt("ollamaUrl", "Ollama 주소")}</div>
          <label className="block">
            <span className="lbl">비전 모델 사용</span>
            <select className="inp" value={d.visionMode} onChange={(e) => set({ visionMode: e.target.value as Settings["visionMode"] })}>
              <option value="fallback">폴백 (DOM 탐색 실패 / Canvas 페이지일 때)</option>
              <option value="always">항상 (SoM 스크린샷 첨부)</option>
              <option value="never">사용 안 함</option>
            </select>
          </label>
          {num("maxAgentSteps", "에이전트 최대 스텝")}
        </div>
        <div className="mb-1 mt-4 text-[12px] font-semibold text-sky-300">실행</div>
        <div className="grid grid-cols-2 gap-3">
          {num("defaultTimeoutMs", "요소 탐색 타임아웃 (ms)", "Tier 1 실패로 판단하기까지의 시간")}
          {num("stepDelayMs", "스텝 간 지연 (ms)")}
        </div>
        <div className="mb-1 mt-4 text-[12px] font-semibold text-sky-300">안전장치</div>
        <div className="space-y-2">
          {chk("confirmSensitive", "에이전트가 결제·구매·삭제 등 민감한 버튼을 누르기 전에 확인받기")}
          {chk("usePlanner", "Planner 사용 (목표를 먼저 계획으로 분해)")}
          {chk("useValidator", "Validator 사용 (완료 선언을 별도 호출로 검증)")}
          {txt("allowedDomains", "에이전트 NAVIGATE 허용 도메인 (쉼표 구분)", "비우면 제한 없음", "example.com, shop.example.org")}
        </div>
        <div className="mt-4 flex justify-end gap-2">
          <button className="btn" onClick={onClose}>
            취소
          </button>
          <button
            className="btn btn-primary"
            onClick={async () => {
              await save(d);
              await refreshModels();
              onClose();
            }}
          >
            저장
          </button>
        </div>
      </div>
    </div>
  );
}
