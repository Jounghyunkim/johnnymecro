# Johnny Mecro 개인정보처리방침

시행일: 〔게시일〕
게시자: JohnnyStudio
문의: 〔연락처 이메일〕

## 요약

Johnny Mecro는 사용자의 PC 안에서만 동작합니다. 게시자는 서버를 운영하지 않으며, 앱은 사용자의 정보를 게시자나 제3자에게 전송하지 않습니다. 사용 통계나 오류 보고도 수집하지 않습니다.

## 앱이 PC에 저장하는 정보

| 정보 | 저장 위치 | 보호 방식 |
| --- | --- | --- |
| 사용자가 등록한 사이트 로그인 아이디와 비밀번호 | 앱 데이터 폴더의 `credentials.dat` | Windows 데이터 보호(DPAPI)로 암호화. 같은 PC의 같은 Windows 계정으로만 복호화 가능 |
| 시나리오(기록된 클릭, 입력값, 주소) | 앱 데이터 폴더의 `scenarios` | 암호화하지 않음. 비밀번호는 참조만 기록되고 값은 포함되지 않음 |
| 앱 설정 | 앱 데이터 폴더의 `settings.json` | 암호화하지 않음 |
| 자동화용 브라우저 프로필(쿠키, 로그인 세션 등) | 앱 데이터 폴더의 `chrome-profile` | 브라우저가 관리 |

이 정보는 앱을 제거하거나 해당 파일을 삭제하면 지워집니다. 로그인 정보는 앱의 "로그인 정보" 탭에서 개별 삭제할 수 있습니다.

## 네트워크 사용

- 앱은 사용자의 PC에서 실행 중인 브라우저(Chrome 또는 Edge)에 로컬 연결로 명령을 보냅니다.
- AI 기능을 사용하면 현재 페이지의 요약된 내용과 화면 캡처가 설정에 지정된 Ollama 서버로 전달됩니다. 기본값은 사용자 PC(`127.0.0.1`)입니다. 사용자가 이 주소를 다른 컴퓨터로 바꾸면 그 컴퓨터로 전달됩니다.
- 비밀번호는 AI에 전달되지 않습니다. 로그인 아이디는 실행 로그와 AI에 전달되는 기록에 포함될 수 있습니다.
- 앱이 여는 웹사이트는 그 사이트의 개인정보처리방침에 따라 사용자의 정보를 처리합니다. 저장된 아이디와 비밀번호는 사용자가 지정한 사이트의 로그인 화면에만 입력됩니다.

## 제3자 소프트웨어

앱은 Google Chrome 또는 Microsoft Edge, 그리고 선택적으로 Ollama와 함께 동작합니다. 이들 소프트웨어의 정보 처리는 각 제공자의 방침을 따릅니다.

## 아동

앱은 아동을 대상으로 하지 않으며 아동의 개인정보를 수집하지 않습니다.

## 방침 변경

방침이 바뀌면 이 페이지에 게시하고 시행일을 갱신합니다.

---

# Johnny Mecro Privacy Policy

Effective date: 〔publication date〕
Publisher: JohnnyStudio
Contact: 〔contact email〕

## Summary

Johnny Mecro runs entirely on your PC. The publisher operates no servers, and the app does not send your information to the publisher or to third parties. It collects no usage statistics or crash reports.

## Information the app stores on your PC

| Information | Location | Protection |
| --- | --- | --- |
| Site sign-in IDs and passwords you register | `credentials.dat` in the app data folder | Encrypted with Windows Data Protection (DPAPI); can be decrypted only by the same Windows account on the same PC |
| Scenarios (recorded clicks, typed values, addresses) | `scenarios` in the app data folder | Not encrypted. Passwords are stored as references only, never as values |
| App settings | `settings.json` in the app data folder | Not encrypted |
| Automation browser profile (cookies, sessions) | `chrome-profile` in the app data folder | Managed by the browser |

This information is removed when you uninstall the app or delete the files. Sign-in details can be deleted individually in the app's sign-in tab.

## Network use

- The app sends commands over a local connection to the browser (Chrome or Edge) running on your PC.
- When you use AI features, a condensed version of the current page and screenshots are passed to the Ollama server set in the app's settings. The default is your own PC (`127.0.0.1`). If you change this address to another computer, the content is sent there.
- Passwords are never passed to the AI. Sign-in IDs may appear in run logs and in the history passed to the AI.
- Websites the app opens handle your information under their own privacy policies. Saved IDs and passwords are typed only into the sign-in screens of the sites you chose.

## Third-party software

The app works with Google Chrome or Microsoft Edge and, optionally, Ollama. Their handling of information is governed by their providers' policies.

## Children

The app is not directed at children and does not collect children's personal information.

## Changes

Changes to this policy will be posted on this page with a new effective date.
