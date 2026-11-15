import { useEffect, useState } from "react";
import { Eye, EyeOff, KeyRound, Pencil, Plus, ShieldCheck, Trash2, X } from "lucide-react";
import { useStore } from "./store";
import type { CredMeta } from "./types";

const hostOf = (site: string) =>
  site.trim().toLowerCase().replace(/^[a-z]+:\/\//, "").split(/[/?#]/)[0].replace(/^www\./, "").replace(/:\d+$/, "");

function PasswordInput({ value, onChange, placeholder, autoFocus }: { value: string; onChange: (v: string) => void; placeholder?: string; autoFocus?: boolean }) {
  const [show, setShow] = useState(false);
  return (
    <div className="relative">
      <input
        className="inp pr-8"
        type={show ? "text" : "password"}
        autoComplete="new-password"
        spellCheck={false}
        value={value}
        placeholder={placeholder}
        autoFocus={autoFocus}
        onChange={(e) => onChange(e.target.value)}
      />
      <button type="button" tabIndex={-1} className="absolute right-2 top-1.5 text-slate-500 hover:text-slate-200" onClick={() => setShow(!show)} title={show ? "숨기기" : "표시"}>
        {show ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
      </button>
    </div>
  );
}

// ---------- vault management tab ----------
export function CredsPanel() {
  const creds = useStore((s) => s.creds);
  const saveCred = useStore((s) => s.saveCred);
  const deleteCred = useStore((s) => s.deleteCred);
  const [edit, setEdit] = useState<{ original?: string; name: string; site: string; username: string; password: string } | null>(null);
  const [confirmDel, setConfirmDel] = useState<string | null>(null);

  const startEdit = (c?: CredMeta) =>
    setEdit(c ? { original: c.name, name: c.name, site: c.site, username: c.username, password: "" } : { name: "", site: "", username: "", password: "" });
  const name = edit ? edit.name.trim() || hostOf(edit.site) : "";
  const canSave = !!edit && !!name && /^[\p{L}\p{N}._-]+$/u.test(name) && (!!edit.original || !!edit.password);

  return (
    <div className="h-full overflow-auto p-3">
      <div className="mb-3 flex items-start gap-2 rounded border border-emerald-800/70 bg-emerald-950/30 p-2 text-[11px] text-emerald-200">
        <ShieldCheck className="mt-0.5 h-4 w-4 shrink-0" />
        <span>
          아이디/비밀번호는 Windows DPAPI로 암호화되어 이 PC의 현재 Windows 계정으로만 복호화할 수 있습니다. 비밀번호는 시나리오 파일·로그·LLM 프롬프트에 기록되지 않으며, 저장 후에는
          화면에 다시 표시되지 않습니다.
        </span>
      </div>

      {creds.length === 0 && !edit && <div className="mb-3 text-slate-500">저장된 로그인 정보가 없습니다. 로그인 과정을 녹화하면 자동으로 보관되고, 여기서 직접 추가할 수도 있습니다.</div>}

      {creds.map((c) => (
        <div key={c.name} className="group mb-1.5 rounded border border-slate-700/70 bg-slate-900/60 px-2.5 py-2">
          <div className="flex items-center gap-2">
            <KeyRound className="h-4 w-4 shrink-0 text-amber-400" />
            <div className="min-w-0 flex-1">
              <div className="truncate text-slate-100">{c.name}</div>
              <div className="truncate text-[11px] text-slate-500">
                {c.site || "(사이트 미지정)"} · {c.username || "(아이디 없음)"} · {c.hasPassword ? "••••••••" : "비밀번호 없음"}
              </div>
            </div>
            {confirmDel === c.name ? (
              <button
                className="shrink-0 rounded bg-rose-600 px-1.5 text-[11px] text-white"
                onMouseLeave={() => setConfirmDel(null)}
                onClick={() => {
                  setConfirmDel(null);
                  void deleteCred(c.name);
                }}
              >
                삭제?
              </button>
            ) : (
              <>
                <button className="text-slate-500 hover:text-slate-100" title="수정" onClick={() => startEdit(c)}>
                  <Pencil className="h-3.5 w-3.5" />
                </button>
                <button className="text-slate-500 hover:text-rose-400" title="삭제" onClick={() => setConfirmDel(c.name)}>
                  <Trash2 className="h-3.5 w-3.5" />
                </button>
              </>
            )}
          </div>
          <div className="selectable mt-1 font-mono text-[11px] text-sky-300/80">
            {`{{cred.${c.name}.username}}`} · {`{{cred.${c.name}.password}}`}
          </div>
        </div>
      ))}

      {edit ? (
        <form
          className="mt-3 rounded border border-slate-700 bg-slate-900 p-3"
          onSubmit={async (e) => {
            e.preventDefault();
            if (!canSave) return;
            if (await saveCred({ original: edit.original, name, site: edit.site, username: edit.username, password: edit.password || undefined })) setEdit(null);
          }}
        >
          <div className="mb-2 flex items-center justify-between text-slate-200">
            <span className="font-semibold">{edit.original ? "로그인 정보 수정" : "로그인 정보 추가"}</span>
            <button type="button" className="text-slate-500 hover:text-slate-200" onClick={() => setEdit(null)}>
              <X className="h-4 w-4" />
            </button>
          </div>
          <label className="mb-2 block">
            <span className="lbl">사이트 (도메인 또는 주소)</span>
            <input className="inp" value={edit.site} placeholder="example.com" autoFocus onChange={(e) => setEdit({ ...edit, site: e.target.value })} />
            <span className="text-[11px] text-slate-500">에이전트가 이 사이트의 로그인 화면을 만나면 자동으로 사용합니다</span>
          </label>
          <label className="mb-2 block">
            <span className="lbl">이름 (시나리오에서 참조할 이름)</span>
            <input className="inp" value={edit.name} placeholder={hostOf(edit.site) || "비우면 사이트 도메인"} onChange={(e) => setEdit({ ...edit, name: e.target.value.replace(/[^\p{L}\p{N}._-]/gu, "") })} />
          </label>
          <label className="mb-2 block">
            <span className="lbl">아이디</span>
            <input className="inp" value={edit.username} autoComplete="off" spellCheck={false} onChange={(e) => setEdit({ ...edit, username: e.target.value })} />
          </label>
          <label className="mb-3 block">
            <span className="lbl">비밀번호</span>
            <PasswordInput value={edit.password} onChange={(v) => setEdit({ ...edit, password: v })} placeholder={edit.original ? "변경하지 않으려면 비워 두세요" : ""} />
          </label>
          <div className="flex justify-end gap-2">
            <button type="button" className="btn" onClick={() => setEdit(null)}>
              취소
            </button>
            <button type="submit" className="btn btn-primary" disabled={!canSave}>
              암호화하여 저장
            </button>
          </div>
        </form>
      ) : (
        <button className="btn mt-2" onClick={() => startEdit()}>
          <Plus className="h-4 w-4" /> 로그인 정보 추가
        </button>
      )}
    </div>
  );
}

// ---------- runtime login prompt ----------
export function CredPromptDialog() {
  const p = useStore((s) => s.credPrompt);
  const answer = useStore((s) => s.answerCredPrompt);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [save, setSave] = useState(true);
  useEffect(() => {
    setUsername(p?.username ?? "");
    setPassword("");
    setSave(true);
  }, [p?.id, p?.username]);
  if (!p) return null;
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <form
        className="w-[420px] rounded-lg border border-sky-600 bg-slate-900 p-4 shadow-2xl"
        onSubmit={(e) => {
          e.preventDefault();
          if (!password) return;
          const a = { username, password, save };
          setPassword("");
          void answer(a);
        }}
      >
        <div className="mb-1 flex items-center gap-2 text-base font-semibold text-sky-300">
          <KeyRound className="h-5 w-5" /> 로그인 정보 입력
        </div>
        <div className="mb-3 whitespace-pre-wrap text-slate-300">{p.message}</div>
        <label className="mb-2 block">
          <span className="lbl">아이디{p.site ? ` — ${p.site}` : ""}</span>
          <input className="inp" value={username} autoFocus={!username} autoComplete="off" spellCheck={false} onChange={(e) => setUsername(e.target.value)} />
        </label>
        <label className="mb-3 block">
          <span className="lbl">비밀번호</span>
          <PasswordInput value={password} onChange={setPassword} autoFocus={!!username} />
        </label>
        <label className="mb-4 flex cursor-pointer items-start gap-2">
          <input type="checkbox" className="mt-0.5 accent-sky-500" checked={save} onChange={(e) => setSave(e.target.checked)} />
          <span>
            <span className="text-slate-200">암호화하여 저장 (다음부터 자동 로그인)</span>
            <span className="block text-[11px] text-slate-500">'{p.name}' 이름으로 Windows DPAPI 암호화 저장소에 보관됩니다. 끄면 이번 실행에만 사용합니다.</span>
          </span>
        </label>
        <div className="flex justify-end gap-2">
          <button
            type="button"
            className="btn"
            onClick={() => {
              setPassword("");
              void answer(null);
            }}
          >
            취소
          </button>
          <button type="submit" className="btn btn-primary" disabled={!password}>
            로그인에 사용
          </button>
        </div>
      </form>
    </div>
  );
}

/** Buttons that insert a credential reference into a "type" step value. */
export function CredInsert({ onInsert }: { onInsert: (ref: string) => void }) {
  const creds = useStore((s) => s.creds);
  const [name, setName] = useState("");
  const sel = creds.find((c) => c.name === name) ?? creds[0];
  if (!creds.length) return <div className="mb-2.5 text-[11px] text-slate-500">로그인 값은 「로그인 정보」 탭에 저장한 뒤 {"{{cred.이름.password}}"} 로 참조하세요.</div>;
  return (
    <div className="mb-2.5 flex items-center gap-1.5">
      <KeyRound className="h-3.5 w-3.5 shrink-0 text-amber-400" />
      <select className="inp !w-auto min-w-0 flex-1" value={sel.name} onChange={(e) => setName(e.target.value)}>
        {creds.map((c) => (
          <option key={c.name} value={c.name}>
            {c.name}
          </option>
        ))}
      </select>
      <button type="button" className="btn shrink-0" onClick={() => onInsert(`{{cred.${sel.name}.username}}`)}>
        아이디
      </button>
      <button type="button" className="btn shrink-0" onClick={() => onInsert(`{{cred.${sel.name}.password}}`)}>
        비밀번호
      </button>
    </div>
  );
}
