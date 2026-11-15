import { useEffect, useState } from "react";
import { useStore } from "./store";
import type { ActionKind, Fingerprint, LoopCfg, Step } from "./types";
import { findStep, newAction, updateStep } from "./steps";
import { CredInsert } from "./Creds";

function Field({ label, children, hint }: { label: string; children: React.ReactNode; hint?: string }) {
  return (
    <label className="mb-2.5 block">
      <span className="lbl">{label}</span>
      {children}
      {hint && <span className="mt-0.5 block text-[11px] text-slate-500">{hint}</span>}
    </label>
  );
}

function Toggle({ label, checked, onChange, hint }: { label: string; checked: boolean; onChange: (v: boolean) => void; hint?: string }) {
  return (
    <label className="mb-2.5 flex cursor-pointer items-start gap-2">
      <input type="checkbox" className="mt-0.5 accent-sky-500" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      <span>
        <span className="text-slate-200">{label}</span>
        {hint && <span className="block text-[11px] text-slate-500">{hint}</span>}
      </span>
    </label>
  );
}

/** Textarea that edits a JSON value; commits only when the text parses. */
function JsonArea({ value, onChange, rows = 5 }: { value: unknown; onChange: (v: unknown) => void; rows?: number }) {
  const [text, setText] = useState(() => (value == null ? "" : JSON.stringify(value, null, 2)));
  const [err, setErr] = useState("");
  useEffect(() => {
    setText(value == null ? "" : JSON.stringify(value, null, 2));
    setErr("");
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [JSON.stringify(value)]);
  return (
    <>
      <textarea
        rows={rows}
        spellCheck={false}
        className={`inp font-mono text-[12px] ${err ? "border-rose-500" : ""}`}
        value={text}
        onChange={(e) => {
          const t = e.target.value;
          setText(t);
          if (!t.trim()) {
            setErr("");
            onChange(null);
            return;
          }
          try {
            onChange(JSON.parse(t));
            setErr("");
          } catch (ex) {
            setErr(String(ex));
          }
        }}
      />
      {err && <span className="block text-[11px] text-rose-400">JSON 오류 (수정 전 값 유지)</span>}
    </>
  );
}

const KINDS: { v: ActionKind; l: string }[] = [
  { v: "navigate", l: "URL 이동" },
  { v: "click", l: "클릭" },
  { v: "type", l: "텍스트 입력" },
  { v: "select", l: "드롭다운 선택" },
  { v: "press_key", l: "키 입력" },
  { v: "scroll", l: "스크롤" },
  { v: "wait", l: "대기" },
];

export function Inspector() {
  const step = useStore((s) => (s.current && s.selectedId ? findStep(s.current.steps, s.selectedId) : null));
  const setSteps = useStore((s) => s.setSteps);
  const locked = useStore((s) => s.running || s.recording);
  const settings = useStore((s) => s.settings);
  if (!step) return <div className="p-4 text-slate-500">왼쪽 타임라인에서 스텝을 선택하면 속성을 편집할 수 있습니다.</div>;

  const up = (patch: Partial<Step>) => setSteps((steps) => updateStep(steps, step.id, patch));
  const upA = (patch: Partial<NonNullable<Step["action"]>>) => up({ action: { ...(step.action ?? newAction("click")), ...patch } });
  const upF = (patch: Partial<Fingerprint>) =>
    up({ fingerprint: { ...(step.fingerprint ?? { primaryCss: "", xpath: "", axRole: "", axName: "", innerText: "" }), ...patch } });
  const upL = (patch: Partial<LoopCfg>) => up({ loop: { ...(step.loop as LoopCfg), ...patch } });
  const a = step.action;
  const fp = step.fingerprint;
  const needsEl = a && (a.kind === "click" || a.kind === "type" || a.kind === "select");

  return (
    <fieldset disabled={locked} className="p-3">
      <Field label="스텝 이름">
        <input className="inp" value={step.name} placeholder="(자동)" onChange={(e) => up({ name: e.target.value })} />
      </Field>

      {step.type === "DETERMINISTIC_ACTION" && a && (
        <>
          <Field label="액션">
            <select
              className="inp"
              value={a.kind}
              onChange={(e) => {
                const kind = e.target.value as ActionKind;
                const el = kind === "click" || kind === "type" || kind === "select";
                up({
                  action: { ...newAction(kind), value: a.value, url: a.url },
                  fingerprint: el ? (fp ?? { primaryCss: "", xpath: "", axRole: "", axName: "", innerText: "" }) : null,
                });
              }}
            >
              {KINDS.map((k) => (
                <option key={k.v} value={k.v}>
                  {k.l}
                </option>
              ))}
            </select>
          </Field>
          {a.kind === "navigate" && (
            <Field label="URL" hint="{{변수}} 사용 가능">
              <input className="inp" value={a.url} placeholder="https://example.com" onChange={(e) => upA({ url: e.target.value })} />
            </Field>
          )}
          {(a.kind === "type" || a.kind === "select") && (
            <Field label={a.kind === "type" ? "입력할 텍스트" : "선택할 값 (value 또는 표시 텍스트)"} hint="{{변수}} 사용 가능 — 예: {{item.name}}">
              <input className="inp" type={a.sensitive ? "password" : "text"} value={a.value} onChange={(e) => upA({ value: e.target.value })} />
            </Field>
          )}
          {a.kind === "type" && <CredInsert onInsert={(ref) => upA({ value: ref, sensitive: false })} />}
          {a.kind === "type" && a.sensitive && (
            <div className="mb-2.5 text-[11px] text-amber-300">이 비밀번호는 시나리오 파일에 평문으로 들어 있습니다. 위에서 저장된 로그인 정보로 바꾸는 것을 권장합니다.</div>
          )}
          {a.kind === "press_key" && (
            <>
              <Field label="키" hint="Enter, Tab, Escape, ArrowDown, PageDown, F5, a …">
                <input className="inp" value={a.key} onChange={(e) => upA({ key: e.target.value })} />
              </Field>
              <div className="mb-2.5 flex gap-4">
                {[
                  { bit: 2, l: "Ctrl" },
                  { bit: 1, l: "Alt" },
                  { bit: 8, l: "Shift" },
                ].map((m) => (
                  <label key={m.l} className="flex items-center gap-1">
                    <input
                      type="checkbox"
                      className="accent-sky-500"
                      checked={(a.modifiers & m.bit) !== 0}
                      onChange={(e) => upA({ modifiers: e.target.checked ? a.modifiers | m.bit : a.modifiers & ~m.bit })}
                    />
                    {m.l}
                  </label>
                ))}
              </div>
            </>
          )}
          {a.kind === "wait" && (
            <Field label="대기 시간 (ms)">
              <input className="inp" type="number" min={0} value={a.ms} onChange={(e) => upA({ ms: Number(e.target.value) || 0 })} />
            </Field>
          )}
          {a.kind === "scroll" && (
            <Field label="스크롤 Y 위치 (px)">
              <input className="inp" type="number" value={a.scrollY} onChange={(e) => upA({ scrollY: Number(e.target.value) || 0 })} />
            </Field>
          )}
          {needsEl && (
            <>
              <div className="mb-1 mt-3 border-t border-slate-800 pt-2 text-[11px] font-semibold text-slate-300">다중 식별자 (Multi-Selector Fingerprint)</div>
              <div className="mb-2 text-[11px] text-slate-500">탐색 순서: primaryCss → axRole+axName → xpath → innerText</div>
              <Field label="primaryCss">
                <input className="inp font-mono text-[12px]" value={fp?.primaryCss ?? ""} onChange={(e) => upF({ primaryCss: e.target.value })} />
              </Field>
              <div className="flex gap-2">
                <div className="w-1/3">
                  <Field label="axRole">
                    <input className="inp" value={fp?.axRole ?? ""} placeholder="button" onChange={(e) => upF({ axRole: e.target.value })} />
                  </Field>
                </div>
                <div className="flex-1">
                  <Field label="axName">
                    <input className="inp" value={fp?.axName ?? ""} onChange={(e) => upF({ axName: e.target.value })} />
                  </Field>
                </div>
              </div>
              <Field label="xpath">
                <input className="inp font-mono text-[12px]" value={fp?.xpath ?? ""} onChange={(e) => upF({ xpath: e.target.value })} />
              </Field>
              <Field label="innerText">
                <input className="inp" value={fp?.innerText ?? ""} onChange={(e) => upF({ innerText: e.target.value })} />
              </Field>
              {fp?.boundingBox && (
                <div className="mb-2.5 text-[11px] text-slate-500">
                  boundingBox: x={fp.boundingBox.x}, y={fp.boundingBox.y}, {fp.boundingBox.width}×{fp.boundingBox.height} (viewport {fp.boundingBox.viewportWidth}×
                  {fp.boundingBox.viewportHeight})
                </div>
              )}
              <div className="flex gap-2">
                <div className="flex-1">
                  <Field label="요소 탐색 타임아웃 (ms)" hint={`0 = 기본값 ${settings?.defaultTimeoutMs ?? 5000}`}>
                    <input className="inp" type="number" min={0} value={step.timeoutMs} onChange={(e) => up({ timeoutMs: Number(e.target.value) || 0 })} />
                  </Field>
                </div>
                <div className="flex-1">
                  <Field label="재시도 횟수">
                    <input className="inp" type="number" min={0} max={10} value={step.retries} onChange={(e) => up({ retries: Number(e.target.value) || 0 })} />
                  </Field>
                </div>
              </div>
              <Toggle
                label="Ollama 자가 치유 폴백 (Tier 2)"
                hint="요소를 못 찾으면 로컬 LLM이 동일 역할의 요소를 찾아 실행하고 셀렉터를 자동 패치합니다"
                checked={step.llmFallback}
                onChange={(v) => up({ llmFallback: v })}
              />
            </>
          )}
        </>
      )}

      {step.type === "LLM_CONDITION" && (
        <>
          <Field label="질문 (현재 화면에 대해)" hint="예: 현재 페이지에 '품절' 표시가 있는가?">
            <textarea className="inp" rows={3} value={step.question} onChange={(e) => up({ question: e.target.value })} />
          </Field>
          <Field label="가능한 답 (쉼표로 구분)" hint="기본 true,false — 다중 분기 예: 재고있음,품절,로그인필요">
            <input
              className="inp"
              defaultValue={step.options.join(",")}
              key={step.id}
              onBlur={(e) => {
                const opts = [...new Set(e.target.value.split(",").map((x) => x.trim()).filter(Boolean))];
                const options = opts.length >= 2 ? opts : ["true", "false"];
                const branches: Record<string, Step[]> = {};
                for (const o of options) branches[o] = step.branches[o] ?? [];
                up({ options, branches });
                e.target.value = options.join(",");
              }}
            />
          </Field>
          <div className="text-[11px] text-slate-500">각 답에 대한 분기 스텝은 타임라인의 분기 영역에 추가하세요. 결과는 {"{{last_condition}}"} 변수에도 저장됩니다.</div>
        </>
      )}

      {step.type === "LLM_EXTRACT" && (
        <>
          <Field label="추출 지시">
            <textarea className="inp" rows={3} value={step.instruction} placeholder="검색 결과의 상품명과 가격을 모두 추출" onChange={(e) => up({ instruction: e.target.value })} />
          </Field>
          <Field label="출력 스키마 (JSON)" hint='간단 표기: {"title":"string","price":"number"} · 목록: [{"title":"string"}] · JSON Schema도 가능'>
            <JsonArea value={step.schema} onChange={(v) => up({ schema: v })} />
          </Field>
          <Field label="저장할 변수 이름" hint="이후 스텝에서 {{이름}} / {{이름.0.title}} 로 사용">
            <input className="inp" value={step.outputVar} onChange={(e) => up({ outputVar: e.target.value.replace(/[^\w]/g, "") })} />
          </Field>
        </>
      )}

      {step.type === "AI_GOAL_STEP" && (
        <>
          <Field label="목표 (자연어)" hint="예: 날짜 선택 달력에서 다음 주 금요일 선택">
            <textarea className="inp" rows={4} value={step.goal} onChange={(e) => up({ goal: e.target.value })} />
          </Field>
          <Field label="최대 에이전트 스텝" hint={`0 = 기본값 ${settings?.maxAgentSteps ?? 25}`}>
            <input className="inp" type="number" min={0} max={100} value={step.maxSteps} onChange={(e) => up({ maxSteps: Number(e.target.value) || 0 })} />
          </Field>
          <Toggle
            label="성공 시 매크로로 자동 변환 (Compile to Macro)"
            hint="첫 실행은 AI가 길을 찾고, 성공한 행동 순서를 일반 스텝으로 치환합니다. 다음 실행부터는 LLM 없이 고속으로 동작합니다"
            checked={step.compileToMacro}
            onChange={(v) => up({ compileToMacro: v })}
          />
        </>
      )}

      {step.type === "LOOP" && step.loop && (
        <>
          <Field label="반복 방식">
            <select className="inp" value={step.loop.mode} onChange={(e) => upL({ mode: e.target.value as LoopCfg["mode"] })}>
              <option value="count">횟수 반복</option>
              <option value="list">목록 변수 순회</option>
              <option value="while">LLM 조건이 참인 동안 (페이지네이션)</option>
            </select>
          </Field>
          {step.loop.mode === "count" && (
            <Field label="반복 횟수" hint="본문에서 {{loop_index}} 사용 가능 (0부터)">
              <input className="inp" type="number" min={1} value={step.loop.count} onChange={(e) => upL({ count: Number(e.target.value) || 1 })} />
            </Field>
          )}
          {step.loop.mode === "list" && (
            <>
              <Field label="목록 변수" hint="LLM 추출 결과(배열) 또는 쉼표로 구분된 시나리오 변수">
                <input className="inp" value={step.loop.listVar} placeholder="extracted" onChange={(e) => upL({ listVar: e.target.value })} />
              </Field>
              <Field label="항목 변수 이름" hint="본문에서 {{item}} / {{item.title}} 로 사용">
                <input className="inp" value={step.loop.itemVar} onChange={(e) => upL({ itemVar: e.target.value.replace(/[^\w]/g, "") })} />
              </Field>
            </>
          )}
          {step.loop.mode === "while" && (
            <Field label="계속 조건 (LLM 질문)" hint="예: 활성화된 '다음 페이지' 버튼이 있는가?">
              <textarea className="inp" rows={2} value={step.loop.condition} onChange={(e) => upL({ condition: e.target.value })} />
            </Field>
          )}
          <Field label="최대 반복 횟수 (안전장치)">
            <input className="inp" type="number" min={1} value={step.loop.maxIterations} onChange={(e) => upL({ maxIterations: Number(e.target.value) || 1 })} />
          </Field>
        </>
      )}

      <div className="mt-3 border-t border-slate-800 pt-3">
        <Toggle label="이 스텝 사용" checked={step.enabled} onChange={(v) => up({ enabled: v })} />
        <Toggle
          label="실패해도 다음 스텝 계속 진행"
          checked={step.onError === "continue"}
          onChange={(v) => up({ onError: v ? "continue" : "stop" })}
        />
      </div>
    </fieldset>
  );
}
