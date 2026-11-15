import { useState } from "react";
import {
  Bot, Check, Clock, Copy, Database, GitBranch, Globe, GripVertical, Keyboard, LoaderCircle, MousePointerClick, MoveVertical,
  Repeat, SkipForward, Trash2, TriangleAlert, Type, Wrench, X, ListFilter,
} from "lucide-react";
import { useStore } from "./store";
import type { Step } from "./types";
import { cloneStep, insertAfter, insertStep, moveStep, removeStep, stepTitle, updateStep } from "./steps";
import { AddMenu } from "./AddMenu";

function StepIcon({ step }: { step: Step }) {
  const c = "h-4 w-4 shrink-0";
  switch (step.type) {
    case "LLM_CONDITION":
      return <GitBranch className={`${c} text-amber-400`} />;
    case "LLM_EXTRACT":
      return <Database className={`${c} text-emerald-400`} />;
    case "AI_GOAL_STEP":
      return <Bot className={`${c} text-fuchsia-400`} />;
    case "LOOP":
      return <Repeat className={`${c} text-cyan-400`} />;
    default:
      switch (step.action?.kind) {
        case "navigate":
          return <Globe className={`${c} text-sky-400`} />;
        case "type":
          return <Type className={`${c} text-sky-400`} />;
        case "press_key":
          return <Keyboard className={`${c} text-sky-400`} />;
        case "wait":
          return <Clock className={`${c} text-sky-400`} />;
        case "scroll":
          return <MoveVertical className={`${c} text-sky-400`} />;
        case "select":
          return <ListFilter className={`${c} text-sky-400`} />;
        default:
          return <MousePointerClick className={`${c} text-sky-400`} />;
      }
  }
}

const TYPE_LABEL: Record<string, string> = {
  DETERMINISTIC_ACTION: "액션",
  LLM_CONDITION: "LLM 조건",
  LLM_EXTRACT: "LLM 추출",
  AI_GOAL_STEP: "AI 목표",
  LOOP: "반복",
};

function StatusBadge({ id }: { id: string }) {
  const st = useStore((s) => s.stepStatus[id]);
  if (!st) return null;
  const map = {
    running: <LoaderCircle className="h-4 w-4 animate-spin text-sky-400" />,
    healing: <Wrench className="h-4 w-4 animate-pulse text-amber-400" />,
    ok: <Check className="h-4 w-4 text-emerald-400" />,
    healed: <Wrench className="h-4 w-4 text-amber-400" />,
    failed: <X className="h-4 w-4 text-rose-400" />,
    skipped: <SkipForward className="h-4 w-4 text-slate-500" />,
  } as const;
  return <span title={st.message}>{map[st.status]}</span>;
}

let dragId: string | null = null;

function DropZone({ containerId, beforeId, tall }: { containerId: string; beforeId?: string; tall?: boolean }) {
  const [over, setOver] = useState(false);
  const setSteps = useStore((s) => s.setSteps);
  return (
    <div
      className={`${tall ? "h-6" : "h-1.5"} rounded transition-colors ${over ? "bg-sky-500/70" : ""}`}
      onDragOver={(e) => {
        if (!dragId) return;
        e.preventDefault();
        setOver(true);
      }}
      onDragLeave={() => setOver(false)}
      onDrop={(e) => {
        e.preventDefault();
        setOver(false);
        const id = dragId;
        dragId = null;
        if (id) setSteps((steps) => moveStep(steps, id, containerId, beforeId));
      }}
    />
  );
}

function StepRow({ step, index }: { step: Step; index: number }) {
  const selected = useStore((s) => s.selectedId === step.id);
  const select = useStore((s) => s.select);
  const setSteps = useStore((s) => s.setSteps);
  const locked = useStore((s) => s.running || s.recording);
  const st = useStore((s) => s.stepStatus[step.id]);
  const a = step.action;
  const sub =
    step.type === "DETERMINISTIC_ACTION"
      ? a?.kind === "type" || a?.kind === "select"
        ? `"${a.sensitive ? "•".repeat(Math.min(a.value.length, 10)) : a.value}"`
        : a?.kind === "click"
          ? step.fingerprint?.primaryCss || ""
          : ""
      : step.type === "LLM_EXTRACT"
        ? `→ {{${step.outputVar || "extracted"}}}`
        : step.type === "AI_GOAL_STEP" && step.compileToMacro
          ? "성공 시 매크로로 변환"
          : step.type === "LOOP"
            ? step.loop?.mode === "list"
              ? `목록 {{${step.loop.listVar}}} 순회`
              : step.loop?.mode === "while"
                ? `조건: ${step.loop.condition}`
                : `${step.loop?.count ?? 1}회`
            : "";
  const border =
    st?.status === "running" || st?.status === "healing"
      ? "border-sky-500"
      : st?.status === "failed"
        ? "border-rose-500"
        : selected
          ? "border-sky-600"
          : "border-slate-700/70";
  return (
    <div
      draggable={!locked}
      onDragStart={(e) => {
        dragId = step.id;
        e.dataTransfer.effectAllowed = "move";
        e.dataTransfer.setData("text/plain", step.id);
        e.stopPropagation();
      }}
      onDragEnd={() => (dragId = null)}
      onClick={(e) => {
        e.stopPropagation();
        select(step.id);
      }}
      className={`group flex items-center gap-2 rounded border ${border} ${selected ? "bg-slate-800" : "bg-slate-850 bg-slate-900/60"} px-2 py-1.5 hover:bg-slate-800 ${step.enabled ? "" : "opacity-45"}`}
    >
      <GripVertical className="h-4 w-4 shrink-0 cursor-grab text-slate-600" />
      <span className="w-5 shrink-0 text-right text-[11px] text-slate-500">{index + 1}</span>
      <StepIcon step={step} />
      <div className="min-w-0 flex-1">
        <div className="truncate text-slate-100">{stepTitle(step)}</div>
        {(sub || st?.message) && (
          <div className={`truncate text-[11px] ${st?.status === "failed" ? "text-rose-400" : st?.status === "healed" ? "text-amber-400" : "text-slate-500"}`}>
            {st?.message || sub}
          </div>
        )}
      </div>
      <span className="shrink-0 rounded bg-slate-800 px-1.5 py-0.5 text-[10px] text-slate-400">{TYPE_LABEL[step.type]}</span>
      {step.type === "DETERMINISTIC_ACTION" && step.fingerprint && !step.llmFallback && (
        <span title="LLM 폴백 꺼짐">
          <TriangleAlert className="h-3.5 w-3.5 text-slate-500" />
        </span>
      )}
      <StatusBadge id={step.id} />
      {!locked && (
        <div className="hidden shrink-0 items-center gap-1 group-hover:flex">
          <button
            title="복제"
            className="text-slate-400 hover:text-slate-100"
            onClick={(e) => {
              e.stopPropagation();
              setSteps((steps) => insertAfter(steps, step.id, cloneStep(step)));
            }}
          >
            <Copy className="h-3.5 w-3.5" />
          </button>
          <button
            title={step.enabled ? "비활성화" : "활성화"}
            className="text-slate-400 hover:text-slate-100"
            onClick={(e) => {
              e.stopPropagation();
              setSteps((steps) => updateStep(steps, step.id, { enabled: !step.enabled }));
            }}
          >
            <SkipForward className="h-3.5 w-3.5" />
          </button>
          <button
            title="삭제"
            className="text-slate-400 hover:text-rose-400"
            onClick={(e) => {
              e.stopPropagation();
              setSteps((steps) => removeStep(steps, step.id));
            }}
          >
            <Trash2 className="h-3.5 w-3.5" />
          </button>
        </div>
      )}
    </div>
  );
}

export function StepList({ steps, containerId }: { steps: Step[]; containerId: string }) {
  return (
    <div>
      {steps.map((s, i) => (
        <div key={s.id}>
          <DropZone containerId={containerId} beforeId={s.id} />
          <StepRow step={s} index={i} />
          {s.type === "LOOP" && (
            <div className="ml-6 mt-1 border-l-2 border-cyan-700/60 pl-2">
              <ContainerHead label="반복 본문" color="text-cyan-400" containerId={`${s.id}:children`} />
              <StepList steps={s.children} containerId={`${s.id}:children`} />
            </div>
          )}
          {s.type === "LLM_CONDITION" &&
            s.options.map((o) => (
              <div key={o} className="ml-6 mt-1 border-l-2 border-amber-700/60 pl-2">
                <ContainerHead label={`답이 「${o}」이면`} color="text-amber-400" containerId={`${s.id}:branch:${o}`} />
                <StepList steps={s.branches[o] ?? []} containerId={`${s.id}:branch:${o}`} />
              </div>
            ))}
        </div>
      ))}
      <DropZone containerId={containerId} tall={steps.length === 0 || containerId === "root"} />
    </div>
  );
}

function ContainerHead({ label, color, containerId }: { label: string; color: string; containerId: string }) {
  const setSteps = useStore((s) => s.setSteps);
  const select = useStore((s) => s.select);
  const locked = useStore((s) => s.running || s.recording);
  return (
    <div className={`flex items-center gap-1 text-[11px] ${color}`}>
      {label}
      <AddMenu
        small
        disabled={locked}
        onAdd={(step) => {
          setSteps((steps) => insertStep(steps, containerId, step));
          select(step.id);
        }}
      />
    </div>
  );
}
