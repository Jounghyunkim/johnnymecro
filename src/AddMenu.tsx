import { useEffect, useRef, useState } from "react";
import { Bot, Clock, Database, GitBranch, Globe, Keyboard, MousePointerClick, MoveVertical, Plus, Repeat, Type, ListFilter } from "lucide-react";
import type { Step } from "./types";
import { newStep } from "./steps";

const ITEMS: { label: string; desc: string; icon: React.ReactNode; make: () => Step }[] = [
  { label: "URL 이동", desc: "지정한 주소로 이동", icon: <Globe className="h-4 w-4 text-sky-400" />, make: () => newStep("DETERMINISTIC_ACTION", "navigate") },
  { label: "클릭", desc: "요소 클릭", icon: <MousePointerClick className="h-4 w-4 text-sky-400" />, make: () => newStep("DETERMINISTIC_ACTION", "click") },
  { label: "텍스트 입력", desc: "입력란에 타이핑 ({{변수}} 가능)", icon: <Type className="h-4 w-4 text-sky-400" />, make: () => newStep("DETERMINISTIC_ACTION", "type") },
  { label: "드롭다운 선택", desc: "<select> 항목 선택", icon: <ListFilter className="h-4 w-4 text-sky-400" />, make: () => newStep("DETERMINISTIC_ACTION", "select") },
  { label: "키 입력", desc: "Enter, Tab, Escape …", icon: <Keyboard className="h-4 w-4 text-sky-400" />, make: () => newStep("DETERMINISTIC_ACTION", "press_key") },
  { label: "스크롤", desc: "지정 위치로 스크롤", icon: <MoveVertical className="h-4 w-4 text-sky-400" />, make: () => newStep("DETERMINISTIC_ACTION", "scroll") },
  { label: "대기", desc: "지정 시간 대기", icon: <Clock className="h-4 w-4 text-sky-400" />, make: () => newStep("DETERMINISTIC_ACTION", "wait") },
  { label: "LLM 조건 분기", desc: "화면 상태를 묻고 분기", icon: <GitBranch className="h-4 w-4 text-amber-400" />, make: () => newStep("LLM_CONDITION") },
  { label: "LLM 데이터 추출", desc: "스키마대로 추출해 변수에 저장", icon: <Database className="h-4 w-4 text-emerald-400" />, make: () => newStep("LLM_EXTRACT") },
  { label: "AI 목표 스텝", desc: "자연어 목표를 에이전트가 수행", icon: <Bot className="h-4 w-4 text-fuchsia-400" />, make: () => newStep("AI_GOAL_STEP") },
  { label: "반복", desc: "횟수 / 목록 / 조건(페이지네이션)", icon: <Repeat className="h-4 w-4 text-cyan-400" />, make: () => newStep("LOOP") },
];

export function AddMenu({ onAdd, small, disabled }: { onAdd: (s: Step) => void; small?: boolean; disabled?: boolean }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const h = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    window.addEventListener("mousedown", h);
    return () => window.removeEventListener("mousedown", h);
  }, [open]);
  return (
    <div className="relative inline-block" ref={ref}>
      {small ? (
        <button disabled={disabled} title="이 안에 스텝 추가" className="rounded px-1 text-slate-400 hover:bg-slate-700 hover:text-slate-100 disabled:opacity-40" onClick={() => setOpen(!open)}>
          <Plus className="h-3.5 w-3.5" />
        </button>
      ) : (
        <button disabled={disabled} className="btn" onClick={() => setOpen(!open)}>
          <Plus className="h-4 w-4" /> 스텝 추가
        </button>
      )}
      {open && (
        <div className="absolute left-0 z-30 mt-1 w-64 rounded border border-slate-700 bg-slate-900 py-1 shadow-xl">
          {ITEMS.map((it) => (
            <button
              key={it.label}
              className="flex w-full items-center gap-2 px-3 py-1.5 text-left hover:bg-slate-800"
              onClick={() => {
                setOpen(false);
                onAdd(it.make());
              }}
            >
              {it.icon}
              <span className="text-slate-100">{it.label}</span>
              <span className="truncate text-[11px] text-slate-500">{it.desc}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
