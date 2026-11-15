# Johnny Mecro — Microsoft Store 등록 정보

Partner Center 제출 화면의 항목 순서대로 정리했습니다. `〔 〕` 안은 직접 채워야 하는 값입니다.

## 1. 제품 ID (Product identity)

| 항목 | 값 |
| --- | --- |
| Package/Identity/Name | `JohnnyStudio.johnny-mecro` |
| Package/Identity/Publisher | `CN=FF44B3F5-BADB-4BBA-9A7C-B4F6B3D0DFA2` |
| Package/Properties/PublisherDisplayName | `JohnnyStudio` |
| Package Family Name | `JohnnyStudio.johnny-mecro_1b00fgvzky5a6` |
| Package SID | `S-1-15-2-2886505791-503707567-3826544805-1175587699-1408895647-4280507982-361715095` |
| Store ID | `9NSJ1P9KT0L9` |
| 스토어 링크 (게시 후) | `https://apps.microsoft.com/detail/9NSJ1P9KT0L9` |
| 앱에서 스토어 열기 | `ms-windows-store://pdp/?productid=9NSJ1P9KT0L9` |

## 2. 패키지 (Packages)

| 항목 | 값 |
| --- | --- |
| 업로드 파일 | `store\JohnnyMecro_1.0.0.0_x64.msix` |
| 버전 | 1.0.0.0 |
| 아키텍처 | x64 |
| 대상 기기 | Windows 10/11 데스크톱 (최소 10.0.17763, 1809) |
| 패키지 언어 | 한국어 (ko-KR) |
| 기능(Capabilities) | `internetClient`, `runFullTrust` |
| 표시 이름 (DisplayName) | `johnny-mecro` — Partner Center에서 예약한 이름과 글자 하나까지 같아야 합니다. `Johnny Mecro`로 표시하려면 제품 관리 > 앱 이름 관리에서 그 이름을 추가로 예약한 뒤 `build-msix.ps1 -DisplayName "Johnny Mecro"`로 다시 빌드합니다 |

## 3. 속성 (Properties)

| 항목 | 값 |
| --- | --- |
| 범주 | 생산성 (Productivity) |
| 개인정보처리방침 URL | 〔`privacy-policy.md` 를 게시한 주소〕 |
| 웹 사이트 | 〔선택〕 |
| 지원 연락처 | 〔이메일 또는 URL〕 |
| 제품 선언 | "이 앱은 개인 정보에 액세스, 수집 또는 전송합니다" → 예 (사용자가 입력한 로그인 정보를 PC에 저장) |

시스템 요구 사항:

| | 최소 | 권장 |
| --- | --- | --- |
| OS | Windows 10 1809 (x64) | Windows 11 |
| 메모리 | 4 GB | 16 GB (로컬 AI 모델 사용 시) |
| 기타 | Google Chrome 또는 Microsoft Edge, WebView2 런타임 | Ollama + 로컬 모델 (AI 기능용) |

## 4. 연령 등급 (Age ratings)

IARC 설문에서 폭력·성적 내용·도박·약물 등은 모두 "아니요"입니다. 앱 자체에는 사용자 간 대화, 콘텐츠 공유, 구매, 위치 공유 기능이 없습니다. 예상 등급은 전체 이용가(3+)입니다.

## 5. 스토어 목록 — 한국어 (ko-KR)

**제품 이름**

Johnny Mecro

**간단한 설명**

크롬에서 하는 반복 작업을 한 번 녹화해 두고 버튼 하나로 다시 실행하세요. 화면이 바뀌어도 로컬 AI가 버튼을 다시 찾아 주고, 목표만 말하면 스스로 수행합니다.

**설명**

```text
※ 이 앱은 Google Chrome(또는 Microsoft Edge)이 설치되어 있어야 동작합니다. AI 기능(자가 치유, 자율 실행, 데이터 추출)을 쓰려면 Ollama와 로컬 모델을 따로 설치해야 합니다. Ollama 없이도 녹화와 재생은 사용할 수 있습니다.

Johnny Mecro는 웹 브라우저에서 반복하는 작업을 자동화하는 Windows 데스크톱 앱입니다.

■ 녹화해서 다시 실행
크롬에서 평소처럼 클릭하고 입력하면 그 과정이 스텝으로 기록됩니다. 기록된 스텝은 순서를 바꾸거나 값을 고칠 수 있고, 반복·조건 분기·변수를 넣어 시나리오로 만들 수 있습니다.

■ 화면이 바뀌어도 계속 동작
사이트가 개편되어 버튼의 위치나 이름이 바뀌면 일반 매크로는 멈춥니다. Johnny Mecro는 요소를 여러 방법으로 기억해 두었다가, 찾지 못하면 로컬 AI에게 같은 역할의 요소를 다시 찾게 하고 시나리오를 스스로 고칩니다.

■ 목표만 말하면 수행
"장바구니에 가장 싼 키보드를 담아줘"처럼 목표를 적으면 AI가 계획을 세우고, 페이지를 보며 한 단계씩 실행하고, 결과를 검증합니다. 성공한 과정은 일반 매크로로 저장해 다음부터는 AI 없이 빠르게 실행할 수 있습니다.

■ AI는 내 PC에서만
AI 기능은 내 PC에서 실행되는 Ollama 모델을 사용합니다. 페이지 내용이 외부 AI 서비스로 전송되지 않습니다.

■ 로그인 정보 보호
아이디와 비밀번호는 Windows 데이터 보호(DPAPI)로 암호화해 PC에만 저장합니다. 시나리오 파일과 실행 로그, AI에 전달되는 내용에는 비밀번호가 들어가지 않습니다. 아이디를 먼저 묻고 다음 화면에서 비밀번호를 묻는 2단계 로그인도 처리합니다.

■ 안전장치
결제·구매·삭제처럼 되돌리기 어려운 버튼은 누르기 전에 확인을 받습니다. 최대 실행 단계, 반복 행동 감지, 이동 가능한 도메인 제한을 지원하고 언제든 중지할 수 있습니다.

알아둘 점
- 최상위 페이지만 다룹니다. iframe 안의 요소, 파일 업로드, 드래그 앤 드롭은 지원하지 않습니다.
- CAPTCHA와 2차 인증은 자동으로 처리하지 않고 사용자에게 넘깁니다.
- AI 기능의 정확도는 사용하는 로컬 모델에 따라 달라집니다.
- 자동화하려는 사이트의 이용 약관을 지켜 사용하세요.
```

**이 버전의 새로운 기능**

```text
첫 출시 버전입니다.
```

**제품 기능** (항목당 200자 이내)

1. 크롬 조작 녹화: 클릭, 입력, 선택, 키, 스크롤, 페이지 이동을 스텝으로 기록
2. 시나리오 편집: 순서 변경, 반복(횟수·목록·조건), 조건 분기, 변수 바인딩
3. 자가 치유: 화면이 바뀌어 요소를 못 찾으면 로컬 AI가 다시 찾고 시나리오를 자동 수정
4. 목표 지향 자율 실행: 자연어 목표를 계획 → 실행 → 검증 순서로 수행
5. 매크로로 변환: AI가 성공한 과정을 저장해 다음부터 AI 없이 고속 실행
6. 데이터 추출: 페이지의 정보를 원하는 형식으로 뽑아 변수에 저장
7. 로컬 AI(Ollama) 사용: 페이지 내용을 외부 AI 서비스로 보내지 않음
8. 로그인 정보 암호화 저장, 2단계 로그인 자동 처리
9. 결제·삭제 등 민감한 동작 전 사용자 확인

**검색어** (최대 7개)

웹 자동화, 매크로, 브라우저 자동화, 크롬 자동화, RPA, 업무 자동화, AI 에이전트

**저작권 및 상표 정보**

© 2026 JohnnyStudio

**추가 시스템 요구 사항 문구**

Google Chrome 또는 Microsoft Edge 필요. AI 기능은 Ollama와 로컬 모델 필요(선택).

## 6. 스토어 목록 — 영어 (en-US, 선택)

앱 화면은 한국어만 지원합니다. 영어 목록을 추가한다면 설명 첫머리에 그 사실을 밝혀야 합니다.

**Product name**

Johnny Mecro

**Short description**

Record what you do in Chrome once and replay it with one click. When a site changes, a local AI model finds the button again, and it can carry out a goal you describe in plain words.

**Description**

```text
Note: this app requires Google Chrome (or Microsoft Edge). AI features (self-healing, autonomous runs, data extraction) also require Ollama and a local model, installed separately; recording and replay work without it. The app's interface is in Korean.

Johnny Mecro is a Windows desktop app that automates repetitive work in a web browser.

Record and replay
Click and type in Chrome as usual and each action becomes a step. Reorder steps, edit values, and add loops, conditions and variables to build a scenario.

Keeps working when the page changes
Ordinary macros break when a site is redesigned. Johnny Mecro remembers each element in several ways, and when it still cannot find one it asks a local AI model for the element that plays the same role, then repairs the scenario.

Give it a goal
Describe a goal such as "add the cheapest keyboard to the cart". The AI plans, acts on the page one step at a time, and checks the result. A successful run can be saved as a plain macro that replays quickly without AI.

AI stays on your PC
AI features use an Ollama model running on your own computer. Page content is not sent to an external AI service.

Protected sign-in details
IDs and passwords are encrypted with Windows Data Protection (DPAPI) and kept on your PC only. Passwords never appear in scenario files, run logs, or what the AI sees. Two-step sign-ins (ID first, password on the next screen) are handled.

Safeguards
Hard-to-undo buttons such as pay, buy or delete need your confirmation first. Step limits, loop detection, a domain allow-list and a stop button are built in.

Good to know
- Works on the top-level page only: no iframes, file uploads, or drag and drop.
- CAPTCHA and two-factor prompts are handed to you.
- AI accuracy depends on the local model you use.
- Follow the terms of service of the sites you automate.
```

**Product features**

1. Records clicks, typing, selections, keys, scrolling and navigation in Chrome as steps
2. Scenario editor with reordering, loops, conditions and variables
3. Self-healing: a local AI model re-finds elements after a page changes and updates the scenario
4. Goal-driven runs: plan, act, then validate from a plain-language goal
5. Compile to macro: save a successful AI run and replay it without AI
6. Data extraction from pages into variables
7. Uses local AI (Ollama); page content is not sent to external AI services
8. Encrypted sign-in storage and two-step sign-in support
9. Confirmation before sensitive actions such as payment or deletion

**Search terms**

web automation, macro recorder, browser automation, chrome automation, RPA, workflow automation, AI agent

## 7. 이미지

| 용도 | 파일 | 크기 |
| --- | --- | --- |
| 스크린샷 1 — 시나리오 편집 화면 | `screenshots\01-scenario.png` | 2732×1536 |
| 스크린샷 2 — AI 자율 실행 | `screenshots\02-agent.png` | 2732×1536 |
| 스크린샷 3 — 로그인 정보 보관 | `screenshots\03-credentials.png` | 2732×1536 |
| 스토어 로고 1:1 | `logos\store-logo-300.png` | 300×300 |
| 스토어 로고 1:1 (대형) | `logos\store-logo-1080.png` | 1080×1080 |

스크린샷 캡션:

1. 크롬에서 한 작업이 스텝으로 기록되고, 순서와 값을 바로 고칠 수 있습니다
2. 목표만 적으면 로컬 AI가 계획하고 실행한 뒤 결과를 검증합니다
3. 아이디와 비밀번호는 암호화해 내 PC에만 보관합니다

## 8. 제출 옵션 (Submission options)

**제한된 기능 사용 사유 — runFullTrust**

```text
Johnny Mecro is a Win32 desktop application (Rust + WebView2) packaged as MSIX. It needs runFullTrust to (1) start the user's installed Google Chrome or Microsoft Edge as a separate process with a dedicated profile folder, (2) control that browser through the Chrome DevTools Protocol on a local loopback port, (3) connect to a local Ollama server on loopback for optional AI features, and (4) encrypt saved sign-in details with Windows DPAPI. These operations are not available to a sandboxed AppContainer app.
```

**인증 담당자용 메모 (Notes for certification)**

```text
Requirements: Google Chrome or Microsoft Edge must be installed. No account or sign-in is needed to use the app. The interface is in Korean.

Basic test without AI:
1. Start the app and click "크롬 실행 / 연결" (Launch / connect Chrome). A Chrome window opens with a profile dedicated to this app.
2. Click "새 시나리오" (New scenario), then "녹화" (Record), click and type on any web page in that Chrome window, then click "녹화 종료" (Stop recording). The actions appear as steps.
3. Click "실행" (Run) to replay the steps.

AI features (self-healing, agent tab, extraction) are optional and need Ollama (https://ollama.com) running locally with a model selected in settings. Without Ollama these features show an error message and the rest of the app keeps working.

The app has no telemetry and makes no network requests of its own other than to the local browser debugging port and the Ollama address set in settings (default 127.0.0.1).
```
