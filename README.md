# Johnny Mecro

크롬 조작 **녹화(매크로)** + **자가 치유 실행** + **로컬 Ollama 기반 목표 지향 자율 실행**을 결합한 Windows x64 웹 자동화 데스크탑 앱입니다.
(`세부구현계획.md` 구현체 — Tauri v2 / Rust + React 19 / TypeScript / Tailwind / Zustand)

## 요구 사항

| 항목 | 비고 |
| --- | --- |
| Windows 10/11 x64 + WebView2 | Windows 11에는 기본 포함 |
| Google Chrome (없으면 Edge) | 앱이 `--remote-debugging-port`로 직접 실행 |
| Ollama (선택) | 없어도 녹화·Tier 1 실행은 동작. LLM 스텝/자가 치유/에이전트에 필요 |
| 빌드 시: Node 20+, Rust(stable, MSVC), VS Build Tools | |

## 실행 / 빌드

```powershell
npm install
npm run tauri dev      # 개발 실행
npm run tauri build    # 릴리스 빌드
```

- 실행 파일: `src-tauri\target\release\johnny-mecro.exe`
- 설치 파일: `src-tauri\target\release\bundle\nsis\Johnny Mecro_1.0.0_x64-setup.exe`

### Microsoft Store 패키지 (MSIX)

```powershell
powershell -ExecutionPolicy Bypass -File scripts\build-msix.ps1              # 앱 빌드 + 패키징
powershell -ExecutionPolicy Bypass -File scripts\build-msix.ps1 -SkipBuild   # 기존 릴리스 exe로 패키징만
powershell -ExecutionPolicy Bypass -File scripts\build-msix.ps1 -SignForTest # 로컬 설치 테스트용 자체 서명본도 생성
```

- 결과물: `store\JohnnyMecro_<버전>.0_x64.msix` (서명 없이 Partner Center에 업로드, 스토어가 서명)
- 패키지 ID·게시자는 `src-tauri\msix\AppxManifest.xml`, 버전은 `tauri.conf.json`의 `version`에서 가져옵니다. 새로 제출할 때마다 버전을 올려야 합니다.
- 스토어 등록 문구·스크린샷·개인정보처리방침 초안은 `store\` 폴더에 있습니다.
- Windows 10/11 SDK(`makeappx.exe`, `makepri.exe`)가 필요합니다.

## 사용법

1. **크롬 실행 / 연결** — 디버깅 포트(기본 9222)에 이미 크롬이 떠 있으면 연결하고, 없으면 앱 전용 프로필로 크롬을 띄웁니다. 전용 프로필은 유지되므로 한 번 로그인하면 세션이 재사용됩니다.
   > Chrome 136+는 *기본* 프로필 폴더에 대한 원격 디버깅을 차단합니다. 다른 프로필을 쓰려면 설정에서 `--user-data-dir` 폴더를 지정하세요.
2. **녹화** — 크롬에서 평소처럼 조작하면 클릭·입력·선택·키·스크롤·주소창 이동이 스텝으로 쌓입니다. **녹화 종료** 후 타임라인에서 드래그로 순서를 바꾸거나 속성을 편집합니다.
3. **스텝 추가** — 액션 외에 `LLM 조건 분기`, `LLM 데이터 추출`, `AI 목표 스텝`, `반복`(횟수/목록/조건)을 넣을 수 있습니다. 값에는 `{{변수}}`, `{{item.name}}` 형태로 바인딩합니다.
4. **실행** — 스텝별 상태(성공 ✓ / 자가 치유 🔧 / 실패 ✕)가 표시됩니다. 자가 치유로 바뀐 셀렉터는 시나리오 JSON에 자동 저장됩니다.
5. **에이전트 탭** — 자연어 목표만 주면 Planner → Navigator → Validator 루프로 수행합니다. 성공하면 **새 시나리오로 저장(Compile to Macro)** 으로 실행 경로를 일반 매크로로 변환해, 다음부터는 LLM 없이 고속 실행됩니다.
6. **DOM 탭** — LLM에 전달되는 증류 DOM과 Set-of-Mark 스크린샷을 미리 봅니다.

7. **로그인 정보 탭** — 아래 "로그인 정보 보관" 참고.

시나리오는 `%APPDATA%\com.johnny.mecro\scenarios\*.json` 에 저장됩니다(사이드바의 폴더 아이콘).

## 로그인 정보 보관

로그인이 필요한 사이트의 아이디/비밀번호는 **Windows DPAPI로 암호화**해 `%APPDATA%\com.johnny.mecro\credentials.dat` 에 저장합니다. 같은 PC의 같은 Windows 계정으로만 복호화됩니다.

| 상황 | 동작 |
| --- | --- |
| 로그인 과정을 녹화 | 비밀번호 입력란에 친 값은 저장소로 들어가고, 시나리오에는 `{{cred.<사이트>.password}}` 참조만 기록됩니다. 바로 앞에 입력한 값은 아이디로 보고 `{{cred.<사이트>.username}}` 으로 바꿉니다 |
| 직접 등록 | **로그인 정보** 탭에서 사이트·아이디·비밀번호를 추가/수정/삭제. 입력 스텝 속성에서 [아이디]/[비밀번호] 버튼으로 참조를 넣습니다 |
| 실행 중 저장된 정보가 없음 | 앱에 **로그인 정보 입력** 창이 떠서 아이디/비밀번호를 받습니다. "암호화하여 저장"을 끄면 그 실행에만 씁니다 |
| 에이전트가 로그인 화면을 만남 | 해당 사이트의 저장된 정보로 로그인하고, 없으면 입력 창을 띄웁니다. 실패하면 한 번 더 묻습니다. 매크로로 변환할 때도 참조만 남습니다 |
| 2단계 로그인 (아이디 → 다음 → 비밀번호) | 에이전트는 아이디만 있는 화면도 로그인 화면으로 인식해 아이디 입력 → Enter(반응이 없으면 [다음] 버튼 클릭) → 비밀번호 화면 대기 → 비밀번호 입력까지 이어서 처리합니다. 입력 창은 한 번만 뜹니다. 녹화할 때는 앞 화면에서 입력한 아이디도 참조로 바뀝니다 |

비밀번호는 저장 후 화면에 다시 표시되지 않으며(수정 시 새로 입력), 시나리오 파일·실행 로그·LLM 프롬프트에 들어가지 않습니다. 아이디는 로그와 LLM 대화 기록에 표시됩니다.
CAPTCHA와 2차 인증(OTP, 휴대폰 승인 등)은 자동 처리하지 않고 사용자에게 넘깁니다.

## 실행 파이프라인

| 단계 | 동작 |
| --- | --- |
| Tier 1 | `primaryCss → axRole+axName → xpath → innerText` 순 탐색, 가시성 확인 후 CDP `Input.*` 실제 입력. 보조 식별자로 찾으면 셀렉터를 갱신 |
| Tier 2 | 타임아웃 시 증류 DOM + 원래 Fingerprint를 Ollama에 전달해 같은 역할의 `[index]`를 찾고, 성공 시 시나리오 패치. 실패하면 SoM 스크린샷으로 비전 모델 재시도 |
| Tier 3 | 목표 지향 에이전트. JSON Schema로 응답을 강제하며 `target_index`는 현재 페이지에 실제 존재하는 인덱스로만 제한 |

안전장치: 최대 스텝, 반복 행동(루프) 감지, 결제·구매·삭제류 버튼 클릭 전 사용자 확인, `NEED_USER_HELP` 시 일시정지, NAVIGATE 도메인 허용 목록, 언제든 중지.

## 소스 구조

```
src-tauri/src/
  browser.rs    CDP 클라이언트(WebSocket, 탭 추적, 입력 이벤트, 스크린샷)
  js/inject.js  페이지 주입 스크립트: 녹화 리스너, Fingerprint, 요소 탐색, DOM 증류, SoM 오버레이
  recorder.rs   페이지 이벤트 → 스텝 변환/병합
  engine.rs     Tier 1/2 실행, LLM 조건·추출, 반복, 변수 치환
  agent.rs      Tier 3 에이전트 + 실행 경로(trace) 기록
  ollama.rs     /api/tags, /api/chat(structured output)
  scenario.rs   시나리오 스키마·저장, 설정
  vault.rs      로그인 정보 저장소(DPAPI 암호화)
  lib.rs        Tauri 커맨드
src/            React UI (store.ts, StepTree.tsx, Inspector.tsx, Panels.tsx, App.tsx)
```

## 알려진 제약

- 최상위 프레임만 녹화/증류합니다(iframe 내부 요소는 미지원).
- 파일 업로드, 드래그 앤 드롭, 우클릭 메뉴는 녹화하지 않습니다.
- 저장소는 Windows 계정 단위로 보호됩니다. 같은 계정으로 실행되는 다른 프로그램까지 막아 주지는 않으며, 다른 PC로 파일을 옮기면 열 수 없습니다.
- 로그인 후 사이트에 입력된 비밀번호는 그 웹페이지가 볼 수 있으므로, 신뢰하는 사이트에서만 사용하세요.
- 에이전트 품질은 로컬 모델에 좌우됩니다. `gemma4:12b`로 검증했습니다.
- 문제 분석용: 환경변수 `JM_DEBUG_DIR=<폴더>` 를 주고 실행하면 LLM 요청/응답이 `llm.log`로 기록됩니다.
- 테스트용: 환경변수 `JM_DATA_DIR=<폴더>` 를 주면 설정·시나리오·로그인 정보·크롬 프로필을 그 폴더에 따로 둡니다.
