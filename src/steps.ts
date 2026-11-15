import type { Action, ActionKind, Step, StepType } from "./types";

export const uid = () => Math.random().toString(36).slice(2, 10) + Date.now().toString(36).slice(-4);

export function newAction(kind: ActionKind): Action {
  return { kind, value: "", key: kind === "press_key" ? "Enter" : "", modifiers: 0, url: "", ms: kind === "wait" ? 1000 : 0, scrollX: 0, scrollY: 0, sensitive: false };
}

export function newStep(type: StepType, kind: ActionKind = "navigate"): Step {
  const base: Step = {
    id: uid(),
    type,
    name: "",
    enabled: true,
    action: null,
    fingerprint: null,
    timeoutMs: 0,
    retries: 0,
    llmFallback: true,
    onError: "stop",
    question: "",
    options: [],
    branches: {},
    instruction: "",
    schema: null,
    outputVar: "",
    goal: "",
    maxSteps: 0,
    compileToMacro: false,
    loop: null,
    children: [],
  };
  switch (type) {
    case "DETERMINISTIC_ACTION":
      base.action = newAction(kind);
      if (kind === "click" || kind === "type" || kind === "select")
        base.fingerprint = { primaryCss: "", xpath: "", axRole: "", axName: "", innerText: "" };
      break;
    case "LLM_CONDITION":
      base.options = ["true", "false"];
      base.branches = { true: [], false: [] };
      break;
    case "LLM_EXTRACT":
      base.outputVar = "extracted";
      base.schema = { title: "string" };
      break;
    case "AI_GOAL_STEP":
      base.compileToMacro = false;
      break;
    case "LOOP":
      base.loop = { mode: "count", count: 3, listVar: "", itemVar: "item", condition: "", maxIterations: 20 };
      break;
  }
  return base;
}

/** Deep copy with fresh ids. */
export function cloneStep(s: Step): Step {
  return {
    ...s,
    id: uid(),
    action: s.action ? { ...s.action } : null,
    fingerprint: s.fingerprint ? JSON.parse(JSON.stringify(s.fingerprint)) : null,
    children: s.children.map(cloneStep),
    branches: Object.fromEntries(Object.entries(s.branches || {}).map(([k, v]) => [k, v.map(cloneStep)])),
  };
}

/** Fill in fields that might be missing in files written by hand / older versions. */
export function normalizeStep(s: Partial<Step>): Step {
  const d = newStep((s.type as StepType) || "DETERMINISTIC_ACTION");
  const out: Step = { ...d, ...s, id: s.id || d.id } as Step;
  out.action = s.action ?? null;
  out.fingerprint = s.fingerprint ?? null;
  out.loop = s.loop ?? (out.type === "LOOP" ? d.loop : null);
  out.options = s.options ?? d.options;
  out.children = (s.children ?? []).map(normalizeStep);
  out.branches = Object.fromEntries(Object.entries(s.branches ?? d.branches).map(([k, v]) => [k, (v ?? []).map(normalizeStep)]));
  if (out.type === "LLM_CONDITION") {
    if (!out.options.length) out.options = ["true", "false"];
    for (const o of out.options) if (!out.branches[o]) out.branches[o] = [];
  }
  return out;
}

export function findStep(steps: Step[], id: string): Step | null {
  for (const s of steps) {
    if (s.id === id) return s;
    const c = findStep(s.children, id);
    if (c) return c;
    for (const b of Object.values(s.branches || {})) {
      const f = findStep(b, id);
      if (f) return f;
    }
  }
  return null;
}

/** Immutable map over every list in the tree. */
function mapLists(steps: Step[], fn: (list: Step[], containerId: string) => Step[], containerId = "root"): Step[] {
  return fn(steps, containerId).map((s) => {
    const hasKids = s.children.length > 0 || s.type === "LOOP";
    const hasBranches = s.type === "LLM_CONDITION";
    if (!hasKids && !hasBranches) return s;
    return {
      ...s,
      children: hasKids ? mapLists(s.children, fn, `${s.id}:children`) : s.children,
      branches: hasBranches
        ? Object.fromEntries(Object.entries(s.branches).map(([k, v]) => [k, mapLists(v, fn, `${s.id}:branch:${k}`)]))
        : s.branches,
    };
  });
}

export function updateStep(steps: Step[], id: string, patch: Partial<Step> | ((s: Step) => Step)): Step[] {
  return mapLists(steps, (list) => list.map((s) => (s.id === id ? (typeof patch === "function" ? patch(s) : { ...s, ...patch }) : s)));
}

export function removeStep(steps: Step[], id: string): Step[] {
  return mapLists(steps, (list) => list.filter((s) => s.id !== id));
}

/** Insert into the container (`root`, `<id>:children`, `<id>:branch:<key>`), before `beforeId` or at the end. */
export function insertStep(steps: Step[], containerId: string, step: Step, beforeId?: string | null): Step[] {
  return mapLists(steps, (list, cid) => {
    if (cid !== containerId) return list;
    const idx = beforeId ? list.findIndex((s) => s.id === beforeId) : -1;
    if (idx < 0) return [...list, step];
    return [...list.slice(0, idx), step, ...list.slice(idx)];
  });
}

export function insertAfter(steps: Step[], afterId: string, step: Step): Step[] {
  return mapLists(steps, (list) => {
    const idx = list.findIndex((s) => s.id === afterId);
    if (idx < 0) return list;
    return [...list.slice(0, idx + 1), step, ...list.slice(idx + 1)];
  });
}

export function moveStep(steps: Step[], id: string, containerId: string, beforeId?: string | null): Step[] {
  const s = findStep(steps, id);
  if (!s || id === beforeId) return steps;
  // never move a step into itself
  const targetOwner = containerId.split(":")[0];
  if (targetOwner === id || (targetOwner !== "root" && findStep([s], targetOwner))) return steps;
  return insertStep(removeStep(steps, id), containerId, s, beforeId);
}

export function countSteps(steps: Step[]): number {
  let n = 0;
  for (const s of steps) {
    n += 1 + countSteps(s.children);
    for (const b of Object.values(s.branches || {})) n += countSteps(b);
  }
  return n;
}

export function stepTitle(s: Step): string {
  if (s.name) return s.name;
  switch (s.type) {
    case "LLM_CONDITION":
      return s.question || "LLM 조건 분기";
    case "LLM_EXTRACT":
      return s.instruction || "LLM 데이터 추출";
    case "AI_GOAL_STEP":
      return s.goal || "AI 목표 스텝";
    case "LOOP":
      return "반복";
    default: {
      const a = s.action;
      if (!a) return "액션";
      switch (a.kind) {
        case "navigate":
          return `이동: ${a.url}`;
        case "wait":
          return `대기 ${a.ms}ms`;
        case "press_key":
          return `키 입력: ${a.key}`;
        case "scroll":
          return `스크롤: ${a.scrollY}px`;
        case "click":
          return `클릭: ${s.fingerprint?.axName || s.fingerprint?.innerText || s.fingerprint?.primaryCss || ""}`;
        case "type":
          return `입력: ${s.fingerprint?.axName || s.fingerprint?.primaryCss || ""}`;
        default:
          return `선택: ${s.fingerprint?.axName || ""}`;
      }
    }
  }
}
