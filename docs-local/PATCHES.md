# LOCAL 琛ヤ竵娓呭崟锛堜笂娓稿悓姝?rebase 鐢級

> 绾﹀畾锛氭墍鏈夋湰鍦版彃鍏ヨ甯?`// LOCAL:` 娉ㄩ噴锛涙柊閰嶇疆瀛楁涓€寰?`#[serde(default)]` 涓旇拷鍔犲湪
> struct 灏鹃儴锛涗笉鏂板 workspace member銆傛瘡娆′笂娓稿悓姝ュ悗鎸夋湰琛ㄩ€愭潯鏍稿鍐茬獊銆?
> 鏈枃浠舵湰韬湪 `docs-local/`锛堜笂娓镐笉瀛樺湪姝ょ洰褰曪紝姘镐笉鍐茬獊锛夈€?

## 璁捐绾︽潫鏉ユ簮锛堝閮ㄤ簨鏁呭鐩橈級

| 鏉ユ簮 | 浜嬫晠 | 鏈湴璁捐瀵瑰簲 |
|---|---|---|
| headroom #746锛坈losed锛宖ix #753锛?| Claude Code 瀵归潪瀹樻柟 base_url 鍏抽棴宸ュ叿寤惰繜鍔犺浇锛?25K 鍩虹嚎 | 鍘嬬缉涓嶇宸ュ叿 schema锛沺roxy 涓嶅仛澶栨寕浠ｇ悊褰㈡€?|
| headroom #2186锛坈losed锛?| CCR 涓诲姩鍥炴敞鏃ф憳瑕侊紝token 鍙嶆定 | 鍘嬬缉妫€绱㈢函 pull 寮忥紙瑙勫垝鏂囨。锛?|
| headroom #1869 / #987锛坥pen锛?| wrap 瀵?opencode 闆惰妭鐪?/ 鎸夊崗璁紡鍘?| 杩涚▼鍐呫€佺粨鏋滃眰鍘嬬缉 |
| qwen-code PR #10995 | `${session_id}` 妯℃澘澶?| 鍚屾妯℃澘璇硶杩?extra_headers |
| 鏈満 rtk 瀹炴祴 | 鍘嬬缉灞傝鎶ユ瀯寤烘垚鍔?| 鏃犳崯瀛楁鐧藉悕鍗?|

## 涓€鏈熻ˉ涓侊紙鎸?crate锛?

### xai-chat-state锛堝け璐ヨ鏁拌处鏈摼锛?
- `src/usage.rs`锛歚UsageTotals.failed_model_calls` 瀛楁 + fold + `UsageLedger::record_main_loop_failure`
- `src/commands.rs`锛歚RecordModelCallFailure` 鍙樹綋锛坄// LOCAL:` 娉ㄩ噴澶勶級
- `src/handle.rs`锛歚record_model_call_failure()`
- `src/actor/mod.rs`锛氬搴?match 鑷?
- `src/actor/mutations.rs`锛歚record_model_call_failure()`锛堝彧璁?session 璐︽湰锛?

### xai-grok-status-line锛坧ayload 鎵╁睍锛屽叏閮?additive + serde default锛?
- `src/context.rs`锛歚StatusLineSessionUsage.reasoning_tokens`锛涙柊 `StatusLineApiCalls`銆?
  `StatusLineTurnPerf`锛沗StatusLineContext.api_calls` / `.perf`
- 娉ㄦ剰锛氫笂娓告枃妗?25-status-line.md 涓庣被鍨嬫湁 doc-sync 娴嬭瘯锛岃嫢涓婃父鏀规鏂囦欢闇€鍚屾琛ヨ〃琛?

### xai-grok-shell
- `src/extensions/notification.rs`锛歚PromptUsageModel.failed_model_calls` +
  `From<&UsageTotals>` 绌峰敖瑙ｆ瀯鎺ョ嚎锛堣澶勪笂娓告敞閲婃湰灏辫姹傛柊瀛楁鏄惧紡鎺ョ嚎锛夛紱
  `is_token_empty` / headless 鎶曞奖 / per-model 琛屼笁澶勭┓灏借В鏋勮ˉ `failed_model_calls: _`
- `src/session/acp_session.rs`锛歚SessionActor.last_turn_api_duration_ms`锛圓tomicU64锛?
  LOCAL锛氫笂涓€璋冪敤 API 鏃堕暱锛屼緵 TPS 鍒嗘瘝锛屼笉鍔?chat-state 鍗忚锛?
- `src/session/acp_session_impl/spawn.rs`锛氫笂杩板瓧娈靛垵濮嬪寲
- `src/session/acp_session_impl/status_line.rs`锛?
  - `build_context_window` 濉?`reasoning_tokens`
  - `build_status_context` 濉?`api_calls` / `perf`
  - 鏂?`build_turn_perf()`锛坰ignals 浼氳瘽鍧?TTFT + actor 涓婄殑 last api 鏃堕暱 鈫?TPS锛岄浂鏂板煁鐐癸級
- `src/session/acp_session_impl/sampler_turn.rs`锛?
  - `expand_session_header_templates()` + 3 涓崟娴嬶紱鍦?`reconstruct` 鐨?
    `inject_url_derived_headers` 涔嬪悗灞曞紑 `${session_id}`
  - `log_terminal_failure()` 寮€澶存寕 `record_model_call_failure(None)`锛堟墍鏈夌粓鎬佸け璐ョ殑鍗曚竴鏀跺彛锛?
  - `record_response_token_usage()` 鍐欏叆 `last_turn_api_duration_ms`
- `src/agent/config.rs`锛氭柊 `NetworkConfig`锛坄[network]` 琛級+ `Config.network` 瀛楁 +
  `Config::default()` 琛ュ瓧娈?+ `resolve_runtime_fields` 寮€澶磋皟 `set_process_proxy`锛堥厤缃姞杞芥椂涓€娆＄畻濂斤級

### xai-grok-config锛圵indows shell 鍚庣鍙€夛級
- `src/shell.rs` LOCAL 娈碉細`SHELL_OVERRIDE` OnceLock + `set_windows_shell_override()`锛?
  `detect_windows_shell` 鐨勬樉寮忚鐩栬鍙栭『搴忔敼涓?config 瑕嗙洊 鈫?`GROK_SHELL` 鈫?鑷姩绾ц仈
- 榛樿绾ц仈涓嶅彉锛歱wsh 鈫?powershell.exe 鈫?Git Bash 鈫?powershell.exe

### xai-grok-shell
- `src/agent/config.rs`锛氭柊 `ShellBackendConfig`锛坄[shell] backend` 琛紝鍊硷細
  `pwsh` | `powershell` | `bash`(=gitbash) | `cmd`锛? `Config.shell` 瀛楁 +
  Default 琛ュ瓧娈?+ `resolve_runtime_fields` 閲?`set_windows_shell_override`
  锛坄#[cfg(windows)]`锛岄厤缃姞杞芥椂涓€娆＄畻濂斤級

### xai-grok-extra-ca锛堣繘绋嬬骇鍑哄彛浠ｇ悊锛?
- `src/lib.rs` LOCAL 娈碉細`ProcessProxyRule` + `set_process_proxy` / `process_proxy_rule`
  锛坋nv 鍥為€€ `GROK_PROXY`銆乣GROK_PROXY_HOSTS`锛? `apply_process_proxy` /
  `apply_process_proxy_blocking`锛坄Proxy::custom` 鎸?host 鍚庣紑鐧藉悕鍗曡矾鐢憋紝loopback
  姘歌繙鐩磋繛锛? `process_proxy_tests`锛? 涓崟娴嬶級
- 榛樿鐧藉悕鍗?`["x.ai", "grok.com"]`锛沗proxy_hosts = []` = 鍏ㄩ儴 host

### xai-proto-build锛圵indows 鏋勫缓淇锛宖ork 蹇呴渶锛?
- `src/lib.rs` `emit_rerun_if_changed`锛歚--dependency_out=/dev/stdout` 涓?
  `--descriptor_set_out=/dev/null` 鏄?Unix 璺緞锛學indows 涓?protoc 鐩存帴 panic銆?
  LOCAL 淇锛歐indows 涓嬭蛋涓存椂鏂囦欢 + `NUL`锛屼緷璧栬鍥炶鍚屼竴鏉¤В鏋愯矾寰勶紱
  鍙嶆枩鏉犺矾寰勫綊涓€鍖栧悗鍐嶅仛 well-known-include 杩囨护
- 閰嶅鐜锛氭湰鏈烘棤 DotSlash锛宍bin/protoc` 鏄崰浣嶈剼鏈€備笅杞界湡瀹?protoc 鍒?
  `D:\data\tools\protoc\bin`锛岀紪璇戞椂鍔?`PATH="/d/data/tools/protoc/bin:$PATH"`

## 宸茬煡闂锛堝揩鐓ц嚜甯︼紝闈炴湰鍦拌ˉ涓佸紩鍏ワ級

`cargo test -p xai-grok-shell`锛堟祴璇?profile锛夊瓨鍦ㄤ笂娓稿揩鐓ц嚜甯︾殑缂栬瘧閿欒锛?
`cargo check`锛坙ib 鏈綋锛変笉鍙楀奖鍝嶏紝鍏ㄩ儴閫氳繃锛?

- `src/leader/transport.rs:256`锛歚String + &String` 鍐欐硶涓庡綋鍓?toolchain 涓嶅吋瀹?
- `src/session/acp_session_tests/tool_layer_images_bridge_tests.rs:15`锛歜ase64 crate API 婕傜Щ
- `tests/common/mod.rs` 绛夛細寮曠敤蹇収涓笉瀛樺湪鐨?`reset_startup_settings_for_tests` 绛夊嚱鏁?
- `xai-grok-tools` `src/computer/local/terminal.rs:4884`锛歚parse_login_env_capture` 缂哄け锛?
  闃诲 tools 鐨?lib test 缂栬瘧锛堟晠 CI 瀵?tools 鍙?check 涓嶈窇娴嬭瘯锛?

杩欎簺闃诲浜?shell 鍐呰仈鍗曟祴锛堝惈 `${session_id}` 妯℃澘娴嬭瘯锛夌殑杩愯锛沜hat-state锛?62锛夈€?
status-line锛?7锛夈€乪xtra-ca锛?5+锛夊崟娴嬪叏閮ㄩ€氳繃銆?

## 鐢ㄦ埛渚ч厤缃ず渚嬶紙config.toml锛?

```toml
[network]
proxy = "http://127.0.0.1:7897"
# proxy_hosts = ["x.ai", "grok.com"]   # 缂虹渷鍗虫鍊硷紱[] = 鍏ㄩ儴 host

[shell]
backend = "bash"   # pwsh | powershell | bash(=gitbash) | cmd锛涚己鐪佽嚜鍔ㄧ骇鑱旓紙pwsh 浼樺厛锛?
```

鐜鍙橀噺绛変环鐗╋細`GROK_SHELL=bash`锛坈onfig 瑕嗙洊浼樺厛浜?env锛夈€?

妯″瀷渚э紙opencode Go 浼氳瘽浜插拰澶寸ず渚嬶級锛?

```toml
[model_providers.ocgo]
base_url = "https://opencode.ai/zen/go/v1"
extra_headers = { "x-opencode-session" = "${session_id}" }
```

## 浜屾湡琛ヤ竵锛堝凡瀹屾垚锛?

### xai-grok-status-line锛堢姸鎬佽 item 鎵╁睍锛屽叏閮?additive锛?
- `src/config.rs`锛歚StatusLineItem` 鏂板 `ApiCalls` / `Perf` 鍙樹綋锛坘ebab-case锛歚api-calls`銆乣perf`锛夛紱
  涓よ€?`varies_mid_turn() == true`锛堝洖鍚堝唴璁℃暟涓?TPS 浼氬彉锛岃闇€ tick 鍒锋柊锛?
- `src/context.rs`锛歚StatusLineApiCalls` 琛?`Copy`锛坈ompose 娑堣垂鐢級

### xai-grok-pager锛堝唴缃姸鎬佽娓叉煋 + stats 瀛愬懡浠わ級
- `src/views/status_line/segments.rs`锛歚compose_builtin` 娓叉煋涓や釜鏂版鈥斺€?
  `鉁?n`锛堟湁澶辫触杩藉姞 `鉁?m` 涓旀暣娈?Warn 鑹茶皟锛夈€乣ttft {ms}ms 路 {tps:.1} tok/s`锛堢己鍝鐪佸摢娈碉級锛?
  瀛楀舰璧?`xai_grok_pager_render::glyphs::{check_mark, ballot_x}`锛堟棫鎺у埗鍙板洖閫€锛?
- `src/views/status_line/segments_tests.rs`锛氫袱涓柊娈电殑鍗曟祴
- `docs/user-guide/25-status-line.md`锛歋et up 琛ㄨˉ `api-calls` / `perf` 琛岋紙doc-sync 娴嬭瘯瑕佹眰锛?
- 鏂?`src/stats_cmd/`锛坄grok stats`锛孴3 鐙珛 CLI锛屽彧璇伙級锛歚--json`銆乣--days N`銆乣--limit N`銆?
  `--model <text>`锛堟ā鍨?id 澶у皬鍐欎笉鏁忔劅瀛愪覆杩囨护锛岃繃婊ゅ悗鏃犲尮閰嶆ā鍨嬬殑浼氳瘽鏁磋娑堝け锛夛紱
  閬嶅巻 `<grok-home>/sessions/**/usage.json`锛堚墹4 灞傦級锛屾寜浼氳瘽 / 鏈湴鏃ワ紙`%Y-%m-%d`锛?
  ISO 鍛紙`%G-W%V`锛夎仛鍚?turns锛坄ended_at` RFC3339锛夛紱涓夎鍥鹃兘甯︽寜妯″瀷鎷嗗垎
  锛圝SON `models` 鏁扮粍 + 浜虹被杈撳嚭 `By model` 琛紝鏈€蹇欐ā鍨嬩紭鍏堬級锛?
  鎴愭湰姹傚拰閬囩己鎶?`+` 灏炬爣锛涢」鐩悕缁?`xai_grok_config::decode_cwd_from_dirname` 杩樺師
- `src/app/cli.rs` + `src/lib.rs` + pager-bin `src/main.rs`锛歚Command::Stats` 鎺ョ嚎
  锛堜袱澶勫懡浠ゅ垎绫诲潡 + 鍒嗗彂鑷傦級

### xai-grok-pager锛堟杩庡睆鍝佺墝瀹氬埗锛氱唺鐚ご logo + 褰╄泲鍓爣棰橈級
- `assets/logo/logo07.txt`锛坒ull tier锛?3x7锛? `logo05.txt`锛坈ompact tier锛? 琛岋級锛?
  Grok 瀛楁爣鎹㈡垚鐔婄尗澶寸洸鏂囩偣闃碉紱`.gitattributes` 寮哄埗 `assets/logo/*.txt` LF
  锛坄include_str!` 鍘熸牱宓屽叆锛孋RLF 浼氭妸 `\r` 甯﹁繘浜岃繘鍒舵覆鏌撴垚鏉傚瓧褰級
- `src/views/welcome/hero_box.rs`锛歚HERO_SUBTITLE` 鏀逛负
  `"Code together, cola together 鈥?thanks for pairing with Panda! (/feedback)"`
  锛堝垵鐗堟帾杈?"Share code & cola" 璇昏捣鏉ュ儚鏇垮埆浜哄浠戒唬鐮侊紝宸叉寜鎰忓浘鏀逛负缁撳鍏卞啓锛?
- `src/app/mod.rs`锛氶€€鍑哄熬閮紙缁堢鎭㈠鍚庛€乣Ok(false)` 鍓嶏級杩藉姞 `FAREWELL`
  甯搁噺鎵撳嵃锛坰tderr锛夆€斺€攈ero 鍓爣棰樹細琚?changelog/鍏憡鎸ゆ帀锛岄€€鍑哄憡鍒蹇呮墦鍗帮紝
  褰╄泲绋冲畾灞曠ず锛沗quit_for_update` / 妯″紡 relaunch 璺緞鍏堜簬鎵撳嵃 return锛屼笉浼氭薄鏌?
- `tools/gen_panda_logo.py`锛氬儚绱犫啋鐩叉枃杞崲鑴氭湰锛?x4 鐐?鏍硷紝U+2800 绌虹櫧鏍硷級锛?
  鐢熸垚涓ゆ。 art 骞朵繚璇?LF锛岀暀浣滀粬浜哄畾鍒跺弬鑰?
- `docs/user-guide/28-welcome-branding.md`锛氭柊澧炲唴缃紩瀵兼枃妗ｏ紙`src/docs.rs`
  `USER_GUIDE` 娉ㄥ唽锛夛紝闈㈠悜 AI agent 鐨?logo/鍓爣棰樺畾鍒堕厤鏂光€斺€斿惎鍔ㄦ椂浼氳В鍖呭埌
  `<grok_home>/docs/user-guide/`锛屽埆浜虹殑 AI 鍙鍒板苟鑷姩澶嶅埢鍚岀被瀹氬埗
- `docs/user-guide/29-local-enhancements.md`锛氭柊澧炲唴缃紩瀵兼枃妗ｏ紝鎶婃湰鍦板寮?
  锛坄[network]` 鍑哄彛浠ｇ悊銆乣[shell]` 鍚庣銆佺姸鎬佽 `api-calls`/`perf`銆?
  `${session_id}` 浼氳瘽浜插拰澶淬€丏eepSeek/GLM `reasoning_content` 鎬濊€冨洖浼狅級
  鍐欐垚"浣滅敤 + 浣曟椂涓诲姩閰嶇疆 + 绀轰緥"鐨?AI 寮曞琛紝鍚俊鍙封啋閰嶇疆瀵圭収琛紱
  渚?AI 瑙ｅ寘鍚庤鍒板苟涓诲姩甯敤鎴烽厤缃?

### 鐘舵€佽鍘熺敓榛樿锛堣剼鏈€€褰癸細`tokens` / `cache` / `think` item + 榛樿寮€鍚級
- `xai-grok-status-line/src/config.rs`锛歚StatusLineItem` 鏂板 `Tokens` / `Cache` /
  `Think`锛坘ebab-case锛歚tokens`銆乣cache`銆乣think`锛宍varies_mid_turn` 鍧?true锛夛紱
  `StatusLineType` 鐨?`#[default]` 浠?`Disabled` 缈诲埌 `Builtin`鈥斺€旂己鐪?section 鍗冲嚭琛岋紱
  `DEFAULT_ITEMS` 鎹㈡垚鏈湴鎸囨爣闆?`[model, api-calls, tokens, cache, think, perf]`
  锛堝榻愬師 `~/.grok/statusline.py` 鐨勬寚鏍囬泦鍚堬紱model 姘歌繙鍙緱锛屾斁鏈€宸﹀仛琛岄閿氱偣锛?
  閬垮厤寮€灞€琛岄鏄┖娈佃烦杩囩殑瑙嗚鎶栧姩锛?
- `xai-grok-status-line/src/context.rs`锛歚StatusLineSessionUsage` 琛?`Copy, Eq`
- `xai-grok-pager/src/views/status_line/segments.rs`锛氫笁涓柊娈碘€斺€?
  `in 47k out 3.2k`锛堢獥鍙ｆ€婚噺缂虹渷鍥為€€ usage 涓夋《鍜岋紝k/M 涓€浣嶅皬鏁板幓灏鹃浂锛夈€?
  `cache 95.7%`锛坈ache_read / 浼氳瘽杈撳叆鎬婚噺锛夈€乣think 28.1%`锛坮easoning / 浼氳瘽杈撳嚭锛夛紱
  鎷夸笉鍒板€兼暣娈甸殣钘忥紙鏂颁細璇濆彧鐢?model锛屼笉鐢诲崰浣嶇锛?
- 娑堣垂鏂硅涔夌炕杞悗鐨勬祴璇曞悓姝ワ細`config_tests.rs`锛坥rphan/off 鏂█鏀规樉寮?Disabled銆?
  鏃?type 杞借嵎鏀?鐢婚粯璁よ + 浠嶆姤瀛ゅ効閿?锛夈€乣metrics_tests.rs`锛坲nset 璁?true銆?
  涓嶅彲鐢昏鏀?Disabled锛夈€乨ispatch `status_line.rs`锛堝悓锛夛紱鏂板
  `token_segments_mirror...` / `a_fresh_session_shows_only...` 鍗曟祴锛?
  `docs/user-guide/25-status-line.md` Set up 琛ㄤ笌榛樿鍊煎悓姝ワ紙doc-sync 娴嬭瘯寮哄埗锛?
- 鐢ㄦ埛鑴氭湰 `~/.grok/statusline.py` 閫€褰瑰彲閫夛細淇濈暀鍗宠鐩栭粯璁わ紙command 浼樺厛锛?

### xai-grok-tools锛圠SP 璇婃柇鍚堝苟鍘绘姈锛?
- `src/implementations/lsp/mod.rs`锛氭柊 `DIAGNOSTICS_QUIET_WINDOW`锛?50ms锛?
- `src/implementations/lsp/manager.rs`锛歚take_answered_diagnostics` 鈫?`take_answered_items`
  锛堣繑鍥?per-file items锛屾牸寮忓寲鏀跺彛鍒版柊 `summary_from`锛夛紱drain 寰幆鏀逛负鎶婂鎵硅鍐?
  绱Н杩涗竴涓?`CollectedDiagnostics`鈥斺€攑ending 鏈竻鍓嶇户缁瓑鏁存壒锛屽叏绛斿畬鍚庢寔 150ms
  闈欓粯绐楀悎骞惰繜鍒扮殑鎺ㄩ€侊紱瓒呮椂/闈欓粯鏀惧純鏃跺凡鏀堕泦鐨勭収鏍峰彂锛堝師鏉ヤ涪寮冿級
- `src/reminders/lsp_diagnostics.rs`锛氭敞鍏ョ骇鍘绘姈鈥斺€擿DrainDebounce{last_inject}` 瀛?
  `SharedResources`锛屾敞鍏ュ悗 750ms 鍐呯殑缂栬緫璺宠繃 drain锛堜笅涓€杞?drain 涓€娆℃姤瀹屾暣鎵癸級锛?
  娑堥櫎杩炵画蹇€熺紪杈戠殑閲嶅娉ㄥ叆涓庨噸澶嶉樆濉?
- 娴嬭瘯锛歚a_drain_merges_staggered_pushes_into_one_summary`锛堥敊宄?200ms 鍙屾帹閫佸悎骞讹級銆?
  `edits_inside_the_debounce_window_share_one_drain`锛團akeBackend 璁℃暟锛?

### xai-grok-pager锛圡CP 鍛藉悕鏍煎紡鏂囨。寮哄寲锛?
- `docs/user-guide/07-mcp-servers.md`锛堜笂娓告枃浠讹紝4 澶勬彃鍏ワ紝鍧囨湁 `<!-- LOCAL: -->` 鎴栧彴璐﹁褰曪級锛?
  - Tool Naming 鏂板 "Server Name Format Requirements" 灏忚妭锛歚<server>__<tool>` 鍏ㄥ悕椤诲尮閰?
    `^[a-zA-Z_][a-zA-Z0-9_-]{0,63}$`锛坄xai-grok-mcp/src/servers.rs` `validate_tool_name`锛夆€斺€?
    server 鍚嶆暟瀛楀紑澶达紙`7zip`锛変細闈欓粯涓㈠厜宸ュ叿锛屽彧鐣?`Skipping MCP tool with invalid name` 鏃ュ織锛?
    `grok mcp add` 鍙煡瀛楃闆嗕笉鏌ラ瀛楃锛屾槸鍧戠殑闅愯斀鐐?
  - Troubleshooting 鏂板 "Server Connects but Its Tools Are Missing" 鐥囩姸鏉＄洰
  - Configuration 寮€澶翠笌 CLI Management breaking-changes 鍙ュ悇琛ヤ竴鍙ヨ鍛?+ `#tool-naming` 浜ゅ弶寮曠敤
- 鑳屾櫙锛氫笂娓搁暅鍍忥紙xai-org/grok-build锛夊叧浜?issue 鍖猴紝姝?bug 鏃犱汉鎶ヨ繃锛涙枃妗ｅ厛琛岋紝浠ｇ爜鏀惧寰呬笂娓?

### xai-grok-pager锛坅gents 寮圭獥灞曠ず鍏ㄩ儴鍐呯疆鍙樹綋 + 闅愯棌妯″瀷/鏉€寮€鍏虫枃妗ｏ級
- `src/views/agents_modal.rs`锛氬垹 `user_visible_builtins()` 绛栧睍闅愯棌鍚嶅崟锛宍build_agent_list`
  鏀逛负閬嶅巻鍏ㄩ儴 `BuiltinAgentName::iter()`銆傝儗鏅細`[agent].name` / `GROK_AGENT` 鍙€変换鎰?
  鍐呯疆鍙樹綋锛堝惈 `grok-build-concise`锛夛紝浣嗗脊绐楀彧灞曠ず 5 涓€斺€擿/agents` 閲屾寜 `s` 浼氭妸閫変腑
  鐨?`grok-build` 鍐欏洖 `[agent].name`锛岄潤榛樿鍐欑敤鎴烽厤缃笖鏃?UI 鍙锛堝疄闄呰俯鍧戜簨鏁咃級銆?
  鍥炲綊娴嬭瘯 `build_agent_list_lists_every_builtin_variant` 闃叉柊鍙樹綋鍐嶈钘?
- `docs/user-guide/26-config-reference.md`锛?
  - features 琛ㄨˉ `turn_transient_retry` 琛岋紙涓婃父婕忔枃妗ｇ殑鍥炲悎鐬椂閲嶈瘯鏉€寮€鍏筹級
  - models 琛ㄥ悗琛?"Hidden vs disabled" 鏁ｆ枃娈碉細hidden 妯″瀷 `-m` 鍙敤銆佸彧鏈?
    `disabled_models` 鎵嶇Щ鍑虹洰褰曘€佸唴缃?plumbing 妯″瀷锛坵eb_search/image_description/
    瀛愪唬鐞?娆¤妯″瀷锛夋寜璁捐闅愯棌

## 鍙戝竷

- `.github/workflows/release.yml`锛氭帹 `v*` tag 瑙﹀彂锛屾瀯寤?`xai-grok-pager`锛坓rok CLI锛?
  release 浜岃繘鍒讹紝浠?linux-amd64锛坱ar.gz锛変笌 windows-amd64锛坺ip锛夛紝闄?sha256銆?
- CI 缂撳瓨浜嬫晠涓庢渶缁堟柟妗堬紙job 鍚嶆挒杞?鈫?sccache GHA 纰庣墖鎾戠垎 10 GB 鈫?鏀瑰洖闅旂鐨?rust-cache锛夛細
  瑙?`ci-cache-incident.md`銆?
## 寰呭姙锛堜笅涓€鏈燂級

- 宸ュ叿杈撳嚭鍘嬬缉锛氫竴鏈熷凡钀藉湴锛堢畝鍖?headroom 绛栫暐锛岃 `tool-output-compression-plan.md`锛夛紱
  浜屾湡浣庢崯/鏃犳崯缁勫悎宸插畾绋匡紙2026-09-17锛岃鍚屾枃妗ｃ€屼簩鏈熸柟鍚戙€嶈妭锛夛紱only-cc-lite git
  渚濊禆涓?embedding 妫€绱粛涓嶅仛
- 鎺掗槦鏃堕棿鎴?/ wait 璁＄畻锛坄acp_session_impl/prompt_queue.rs`锛夛細宸蹭粠璁″垝鍒犻櫎
- 鎺掗槦鍗¤瑙夊尯鍒嗭細宸插簾寮?
- T3 stats 鍚庣画鍙€夐」锛氭妸浼氳瘽琛屾帴 `list_summaries` 鎷挎爣棰樸€乣--project` 杩囨护
- `/stats` 鏂滄潬鍛戒护宸茶惤鍦帮紙鍘嬬缉姝ｅ悜/璐熷悜鏀剁泭 + CCR I/O 寤惰繜锛沗grok stats` 鍚屾灞曠ず锛?

## 涓夋湡琛ヤ竵锛堝凡瀹屾垚锛?026-09-15锛?

> 閫氱敤鍓嶇疆锛堝欢缁級锛氬疄鐜板墠鏍告煡涓婃父鏄惁宸叉湁鐩镐豢/鍐茬獊鏈哄埗锛屾湁鍒欎笉鍋氾紱
> 鍏ㄩ儴鍙厤缃惎鐢?鍋滅敤锛岄粯璁や笉鏀瑰彉涓婃父琛屼负銆?
> 2026-09-15 鏍告煡锛歁CP 鎳掑姞杞戒笂娓稿凡鍐呯疆涓斿己鍒堕粯璁わ紙瑙佸鐓ц〃锛夛紝涓嶅仛銆?
> 鑼冨洿锛氭瀬绠€ primary agent + BeforeModelCall hook + PostCompact 閲嶆敞鍏?+ 閫氱煡寮€绠便€?
> 璇存槑锛歅reCompact/PostCompact 浜嬩欢涓婃父鏈氨瀛樺湪锛坈ompaction.rs 瑙﹀彂锛孫bserve 鍨嬶級锛?
> 涓夋湡琛ョ殑鏄?PostCompact 鐨?additionalContext 閲嶆敞鍏ラ€氶亾銆?

### 宸茶惤鍦?

- `BeforeModelCall` 娑堟伅鍙樻崲 hook锛氭瘡娆?LLM 璇锋眰缁勮瀹屾垚鍚庛€佸彂閫佸墠鏀瑰啓娑堟伅鍒楄〃
  锛堝嚭鍘昏劚鏁?鍥炴潵杩樺師锛屽弻鍚戯級锛宻ession 璁板綍姘镐笉淇敼鈥斺€旇鍓?鑴辨晱/鍘嬬缉绫绘墿灞曠殑
  澶村彿渚濊禆闈紙DCP 4.2k鈽呫€乿ibeguard 鍧囧缓绔嬪湪姝?hook 涓婏級銆傛帴鍏ラ敋鐐癸細
  `acp_session_impl/sampler_turn.rs` 璇锋眰缁勮杈圭晫锛坮econstruct 涔嬪悗銆侀噰鏍疯皟鐢ㄤ箣鍓嶏紝
  fork 鐨?`${session_id}` 妯℃澘灞曞紑宸插湪鍚屼竴 seam锛夛紱浜嬩欢娉ㄥ唽璧?`hook_dispatch.rs`銆?
  瀹冩槸銆屽緟鍔烇紙涓嬩竴鏈燂級銆嶅伐鍏疯緭鍑哄帇缂╃殑鍓嶇疆娑堣垂鑰咃紝鎺掓湡鏃朵竴璧疯瘎浼?
- 鏋佺畝妯″紡 primary agent锛堝鐢ㄧ幇鎴愯浇浣擄紝涓嶆柊澧炶緭鍑洪鏍煎瓙绯荤粺锛夛細涓婃父宸插唴缃?
  concise 鍙樹綋鈥斺€擿BuiltinAgentName::GrokBuildConcise`锛坄xai-grok-agent/src/config.rs`
  `grok_build_concise()`锛歚COMPACT_SYSTEM_PROMPT` + 绮剧畝宸ュ叿鎻忚堪闆?
  `grok_build_concise_toolset` + `agents_md:false`锛夛紝`--agent-profile
  grok-build-concise` / `agent.name` / `/config-agents` 鍧囧彲閫変负涓讳細璇?agent锛?
  浣嗗叾鎻愮ず璇嶅彧鏈変袱鍙ヨ瘽銆侀浂杈撳嚭椋庢牸瑙勫垯銆俧ork 鏀归€?= 缁?concise 杞戒綋杩藉姞 LOCAL
  鏋佺畝瑙勫垯鑺傦細鏀?`xai-grok-agent/src/agent.rs` `system_prompt()` 鐨?concise 鍒嗘敮
  锛坄COMPACT_SYSTEM_PROMPT` 鍚庢嫾鎺?`LOCAL_CONCISE_RULES` 甯搁噺锛夛紝浼氳瘽涓垏鎹㈣矾寰?
  `acp_session_impl/model_switch.rs` 鍚屾鎷兼帴銆傝鍒欒瀺鍚堜笁婧愶細
  qwen Concise锛堢瓟妗堝厛琛屻€侀浂鏃佺櫧闆跺鐩橀浂瀵掓殑銆佺敤鎴疯瑙ｉ噴鏃剁粰鍏ㄦ枃銆佹纭€?鏋佺畝銆?
  鍐茬獊鏃舵湰鑺傝儨鍑猴級+ caveman锛堝幓鍐犺瘝/濉厖璇?瀹㈠璇?瀵瑰啿銆佺煭鍚屼箟璇嶃€佺澶磋〃鍥犳灉銆?
  鐗囨鍙ュ彲鐢ㄣ€佹妧鏈瘝绮剧‘銆佷唬鐮佸潡涓庢姤閿欏師鏂囦笉鍔ㄣ€佸畨鍏ㄨ鍛?涓嶅彲閫嗘搷浣滀复鏃舵仮澶?
  瀹屾暣琛ㄨ揪锛? i-have-adhd 46k鈽咃紙棣栬鍗充笅涓€姝ヨ鍔ㄣ€佸姝ョ紪鍙蜂笖姝ユ暟鏈€灏戙€佹瘡鍥炲悎
  閲嶈堪杩涘害鐘舵€併€佺粨灏捐嚦澶氫竴涓袱鍒嗛挓鍐呭彲鍋氱殑 next action銆侀敊璇钩閾虹洿鍙?
  cause+fix銆佸睍绀哄垪琛ㄢ墹5 鏉′笖鍒嗘瀽瀹屾暣鎬т笉鍙楅檺銆佸畬鎴愮殑浜嬭娓?鐜板湪鑳界敤浠€涔?锛夈€?
  浜屾壒鍊欓€夛細Learning 绫讳氦浜掗鏍笺€乹wen 寮?`keep-coding-instructions` 鍩虹鎻愮ず鍒嗚妭

- `PostCompact` 閲嶆敞鍏ワ紙宸茶惤鍦帮級锛氫笂娓镐簨浠舵湰灏卞瓨鍦紙Observe 鍨嬶紝payload 浠?
  `{source}`锛夛紱涓夋湡琛?`additionalContext` 鍝嶅簲閫氶亾鈥斺€旀敹闆嗘枃鏈湪鍘嬬缉閲嶇疆鍚庝互
  鍗曟潯 system item 閲嶆敞鍏ワ紙`dispatch_post_compact_context` +
  `dispatch_post_compact_collect_context`锛夛紝浠诲姟鍒楄〃/璁板繂绫绘墿灞曠殑
  鐘舵€佹仮澶嶉€氶亾锛坮piv-todo 鏈堜笅杞?14.8 涓囩殑鏍稿績鍗栫偣锛?
- 閫氱煡寮€绠卞寲锛堝凡钀藉湴锛夛細`[notifications]` 閰嶇疆鑺傦紙`desktop`/`sound`/
  `min-interval-secs`/`suppress-after-user-input-secs`锛屽叏閮?opt-in 榛樿鍏筹級锛?
  鍙戝皠璧扮粓绔?OSC 9 + OSC 777 + BEL锛堥浂渚濊禆锛學indows Terminal/iTerm2/kitty/WezTerm锛夛紱
  鑱氱劍鎶戝埗涓哄惎鍙戝紡鈥斺€旂敤鎴锋渶杩?N 绉掓湁杈撳叆鍒欒烦杩囷紱鎸傚湪 `dispatch_notification_hook`
  鍏ュ彛锛屼笌 hooks 瀹屽叏鐙珛

**琛屼负鍙樺寲锛坮ebase 鐢級**锛歡rok-build-concise 鐜颁负 strict harness锛堝畾鍒舵彁绀?+
绮鹃€夊伐鍏烽泦锛夆€斺€斿鎴风 `_meta.agentProfile` 涓嶈兘瑕嗙洊瀹冿紙涓?codex 鍚岀瓥鐣ワ級锛?
`harnesses_are_compatible` 瑙嗗叾浠呬笌鑷韩鍏煎锛屽垏鎹㈠埌瀹冮渶閲嶅缓 harness锛堥噸寤鸿矾寰?
`model_switch.rs` 浼氬啓鍏?compact+瑙勫垯鎻愮ず锛岃涔変竴鑷达級銆傜浉鍏充笂娓告祴璇曞凡鎸夋鏇存柊锛?
mvp_agent/tests.rs锛堝吋瀹圭煩闃?+ ACP profile 瑙ｆ瀽锛夈€亁ai-grok-agent config.rs
锛坄expected_strict_harness` / 鎸夊悕鍒嗙被锛夈€?

**瀹炵幇閿氱偣锛坮ebase 鐢級**锛歚xai-grok-hooks`锛坋vent.rs 浜嬩欢/GateKind::ModelCall/payload銆?
runner/mod.rs `resolve_rewrites`+`gate_outcome(gate)`銆乨ispatcher.rs
`MessageRewrite`/`dispatch_before_model_call`/`dispatch_post_compact_context`銆?
config.rs ModelCall 瓒呮椂锛夛紱`xai-grok-agent`锛坱emplate.rs `LOCAL_CONCISE_RULES`銆?
config.rs `grok_build_concise()` Custom 妯℃澘锛夛紱`xai-hooks-plugins-types`
锛圚ookEvent::BeforeModelCall锛夛紱`xai-grok-shell`锛坱urn.rs 閲囨牱鍓?seam + 澶辫触褰掑洜銆?
hook_dispatch.rs `apply_before_model_call_hooks`/`trip_before_model_call_breaker`
(闃堝€?3)/`dispatch_post_compact_collect_context`銆乵odel_switch.rs 瑙勫垯鎷兼帴銆?
updates.rs `emit_builtin_notification`銆乼ypes.rs 娲诲姩鏃堕棿鎴抽潤鎬併€乤gent/config.rs
`NotificationsConfig`銆乧ompaction.rs PostCompact 閲嶆敞鍏ワ級銆?

## 涓婃父宸叉湁鑳藉姏瀵圭収锛堢ぞ鍖哄懠澹?鈫?鍕块噸澶嶅疄鐜帮級

| 绀惧尯鍛煎０锛堥珮 reaction/楂樻槦锛?| 涓婃父 grok 鐜扮姸 |
|---|---|
| `/context` 涓婁笅鏂囧垎瑙?| 宸叉湁锛堢粏鍒嗗埌 tool defs / skills / MCP 鎴愭湰锛?|
| `/goal` 鎸佷箙鐩爣 + token 棰勭畻 | 宸叉湁锛坄--budget` + 瀵规姉楠岃瘉锛?|
| `/btw` 渚ч棶娴眰 | 宸叉湁锛坄/aside`锛屽惈 minimal 闈㈡澘锛?|
| 缁撴瀯鍖栨彁闂伐鍏?| 宸叉湁锛堝唴缃?`ask_user_question`锛?|
| LSP 璇婃柇鍥炲杺 | 宸叉湁锛坮epo 绾?server + 鎻掍欢 LSP + `lsp` 宸ュ叿锛涙湰鍦板凡鍋氬悎骞跺幓鎶栧寮猴級 |
| subagent 缂栨帓 | 宸叉湁锛坧ersonas + `send_subagent_message` + monitor/kill_task锛?|
| worktree 闅旂 | 宸叉湁锛堝唴缃級 |
| 鏉冮檺鐭╅樀 / sandbox / headless / memory / hooks | 鍧囨湁锛坔ooks 鍚?`updatedInput` 鏀瑰弬銆乣updatedToolOutput` 鏇挎崲銆乺ecord 涓?model 鍒嗙锛?|
| 浜や簰寮忓悗鍙拌繘绋?| 宸叉湁锛坆ackground tasks + ptyctl锛?|
| 閫氱煡 | 浜嬩欢宸叉湁锛岀己寮€绠卞疄鐜帮紙宸插垪涓夋湡 P1锛?|
| MCP 鎳掑姞杞斤紙pi-mcp-adapter 鏈堜笅杞?94 涓囷級 | 宸叉湁涓斿己鍒堕粯璁わ細璇锋眰 tools 鏁扮粍鍙惈鍐呯疆宸ュ叿锛坄sampler_turn.rs` `prepare_tool_definitions_inner` 鈫?`tool_definitions_builtins_only`锛屾敞閲?"tool search is always enabled"锛夛紝MCP 宸ュ叿璧?`search_tool`锛圔M25 绱㈠紩锛夆啋 `use_tool` 涓ゆ寮忥紱announcement 鏄彉鏇存椂澧為噺 `<system-reminder>`锛堟寚绾规寔涔呭寲 `announcement_state.json`锛宍MCP_REMINDER_MODE`=delta/full锛夛紝涓嶅仛鍏ㄩ噺 schema 甯搁┗ |
| opencode primary 鑷畾涔?agent锛圱ab 鍒囨崲 build/plan锛?| 宸叉湁锛歛gent 瀹氫箟 `.grok/agents/*.md` / `~/.grok/agents/`锛堜綔鐢ㄥ煙鍚富浼氳瘽锛歮odel/tools/prompt body/skills锛夛紝`/config-agents` 璁鹃粯璁?+ 浼氳瘽涓垏鎹㈡縺娲伙紝鍚姩渚?`--agent-profile` / `GROK_AGENT` / `agent.name`锛沺ersonas 鏄?subagent 涓撳睘琛屼负鍙犲姞灞?|
| 杈撳嚭椋庢牸鏋佺畝/璇︾粏锛坬wen `/output-style` 澶氶鏍奸€夋嫨鍣級 | 涓嶇収鎼紙鐢ㄦ埛鍐崇瓥锛氬彧鍋氭瀬绠€涓€绉嶏級銆傛瀬绠€涓?agent 涓婃父宸叉湁杞戒綋锛氬唴缃?`grok-build-concise`锛坄--agent-profile`/`agent.name`/`/config-agents` 鍙€夛紝`COMPACT_SYSTEM_PROMPT` + 绮剧畝宸ュ叿闆嗭級锛屼絾鎻愮ず璇嶆棤椋庢牸瑙勫垯 鈫?fork 娉ㄥ叆涓夋簮铻嶅悎瑙勫垯鑺傦紝宸插垪涓夋湡 P0 |

## 鍥涙湡琛ヤ竵锛圱UI 鐣岄潰鏂囨鍙岃锛?

### xai-grok-pager锛堟枩鏉犲懡浠ゆ弿杩?鐢ㄦ硶涓嫳鍒囨崲锛岄粯璁や腑鏂囷級
- `src/slash/i18n.rs`锛堟柊鏂囦欢锛夛細`Lang`锛圸h 榛樿 / En锛? 杩涚▼绾?`AtomicU8` 鍏ㄥ眬鐘舵€?
  锛堥璇绘椂 `GROK_LANG=en` 鍙敼榛樿锛? `tr()`锛堣嫳鏂囧師鏂?鈫?涓枃璇戞枃鏌ヨ〃锛屾棤璇戞枃/鑻辨枃妯″紡
  鍘熸牱閫忎紶锛? `translations()` 闈欐€佺炕璇戣〃锛堢害 90 缁勶細鍏ㄩ儴鍐呯疆鍛戒护 description/usage/
  arg_placeholder銆乪ffort 绛夌骇鎻忚堪銆乿oice/minimal/fullscreen 鎵嬪啓鏂囨锛? `test_sync`
  娴嬭瘯涓茶閿?
- `src/slash/command.rs`锛歚slash_meta!` 瀹忕殑 `description` / `usage` / `arg_placeholder`
  涓変釜鐢熸垚浣嶅寘涓€灞?`i18n::tr()`锛圠OCAL 娉ㄩ噴澶勶級锛涘叾浣欏瓧娈典笉鍔?
- `src/slash/commands/voice.rs`銆乣screen_mode_switch.rs`锛氭墜鍐?`description()` 涓ゅ瀛楅潰閲?
  杩?`tr()`锛沗effort_levels.rs` `effort_description()` 鍚勫垎鏀繃 `tr()`
- `src/slash/commands/lang.rs`锛堟柊鍛戒护 `/lang`锛夛細鏃犲弬鏁板湪涓嫳闂村垏鎹紝`zh|en|涓枃|english`
  鏄惧紡鎸囧畾锛屾湭鐭ュ弬鏁版姤閿欙紱鍒囨崲鍚庤繑鍥炵‘璁ゆ秷鎭€傚凡娉ㄥ唽杩?`commands/mod.rs` `builtin_commands()`
- `src/slash/registry.rs`锛氭柊澧?`pub refresh_trigger_text()`锛堣浆鍙戠鏈?`rebuild_triggers()`锛?
- `src/app/dispatch/prompt.rs` + `dashboard.rs`锛氫袱澶勫懡浠ゅ垎鍙戠偣鍦ㄦ墽琛屽墠璁板綍璇█銆佹墽琛屽悗
  鑻ヨ瑷€鍙樺寲鍒欏鍚勮嚜 slash_controller 鐨?registry 璋?`refresh_trigger_text()`锛?
  浣挎枩鏉犺彍鍗?鍛戒护闈㈡澘/ghost 琛ュ叏鐨勬弿杩版枃鏈珛鍗虫崲璇█
- 杈圭晫锛欰CP/鎶€鑳界瓑杩愯鏃舵枃妗堜笉缈昏瘧锛堣〃澶栭€忎紶锛夛紱璇█涓嶆寔涔呭寲鍒?config.toml锛?
  浼氳瘽鍐呮湁鏁堬紝`GROK_LANG=en` 鍙浐瀹氳嫳鏂?
- 娴嬭瘯璇箟锛歚cfg!(test)` 鏋勫缓涓?tr() 榛樿鑻辨枃锛堜笂娓告棦鏈夋祴璇曟寜鑻辨枃鏂囨鏂█锛夛紝
  鐢熶骇榛樿涓枃锛沗xai-grok-shell` `slash_commands.rs` `PAGER_COMMAND_KEYS` 杩藉姞
  `"lang"` 鍗犱綅锛堥槻鎶€鑳藉悓鍚嶉伄钄斤紝娴嬭瘯 `pager_builtin_triggers_are_reserved_in_shell` 寮哄埗锛?

## 浜旀湡琛ヤ竵锛?026-09-17锛氱姸鎬佽鏁版嵁淇 + i18n 瑕嗙洊鎵╁睍 + 鑷姩鏇存柊榛樿鍏筹級

### xai-grok-shell锛堢姸鎬佽 in 0 out 0 淇锛?
- `src/session/acp_session_impl/status_line.rs`锛氱┖璐︽湰锛坄model_calls == 0`锛夋姇褰卞嚭鐨?
  `UsageTotals` 鍏ㄩ浂锛宍session_input_tokens = Some(0)` 浣?tokens 娈典粠浼氳瘽涓€寮€濮嬪氨鐢诲嚭
  `in 0 out 0`锛堜笌"鏁版嵁瀛樺湪鎵嶇粯鍒?鐨勮璁＄浉鎮栵級銆備慨澶嶏細`build_status_context` 閲岀粰
  `build_context_window` 浼?`window_totals = totals.filter(|t| t.model_calls > 0)`锛?
  鏃犺皟鐢ㄦ椂 token 绐楀彛鏁翠綋缂哄腑锛岃鍙敾 model锛沗api_calls`/`cost` 浠嶇敤鍘熷 totals
  锛堝墠鑰呰嚜甯?`model_calls > 0 || failed > 0` 杩囨护锛?
- `src/session/acp_session_impl/sampler_turn.rs`锛歚record_response_token_usage` 璁拌处鍚?
  杩藉姞 `emit_status_snapshot_detached()`锛圠OCAL锛夛紝姣忔妯″瀷鍝嶅簲绔嬪嵆鍒锋柊鐘舵€佽锛?
  涓嶅啀鍙瓑 turn-end 蹇収

### xai-grok-pager锛坕18n 瑕嗙洊鎵╁睍锛氬揩鎹烽敭鏍?+ shell ACP 鍛戒护锛?
- `src/slash/i18n.rs`锛氭柊澧?`tr_str(&str) -> String`锛堝姩鎬佸瓧绗︿覆鏌ヨ〃锛涜嫳鏂囨ā寮?琛ㄥ
  鍘熸牱杩斿洖锛夛紱缈昏瘧琛ㄨ拷鍔狅細蹇嵎閿彁绀烘爮鍏ㄩ儴鏍囩锛坰end/cancel/copy plan 绛夌害 75 缁勶紝
  娓叉煋鏃舵煡琛級+ "press again to" 鍓嶇紑 + shell 鍐呯疆鍛戒护鎻忚堪/鍗犱綅绗︼紙/memory /flush
  /dream /context /hooks-* /session-info /deep-research /goal /plugins 绛夌害 30 缁勶級
- `src/views/shortcuts_bar.rs`锛歜ar 娓叉煋澶勬爣绛句笌 "press again to {label}" 鍓嶇紑缁?
  `tr_str`/`tr` 鏌ヨ〃锛屽搴︽寜璇戞枃璁?
- `src/slash/acp_command.rs`锛歚AcpSlashCommand::from` 鏋勯€犳椂瀵?ACP 涓嬪彂鐨?
  description / arg_hint 杩?`tr_str`锛堣鐩?shell 绔懡浠ゅ湪鏂滄潬鑿滃崟閲岀殑涓枃鏄剧ず锛?

### xai-grok-update + xai-grok-pager-bin锛堣嚜鍔ㄦ洿鏂伴粯璁ゅ叧闂級
- 鑳屾櫙锛氬畼鏂瑰畨瑁呭櫒鑷姩鍗囩骇浼氭妸 fork 鏋勫缓瑕嗙洊涓哄畼鏂逛簩杩涘埗锛?026-09-17 瀹炶瘉锛氭湰鍦?
  fork 1.0.29 琚鐩栦负瀹樻柟 1.0.34锛?
- `src/auto_update.rs`锛歚check_update_background` / `run_update_if_available` 鐨?
  auto_update 闂ㄤ粠 `== Some(false)` 鎷︽埅鏀逛负 `!= Some(true)` 鎷︽埅锛圢one 榛樿鍏筹級锛?
  鍒犻櫎棣栧啓 `Some(true)` 鐨勬寔涔呭寲锛沗UserCommand` 瑙﹀彂锛堟墜鍔?`grok update`锛変笉鍙楅棬闄?
- `xai-grok-pager-bin/src/main.rs`锛歭eader 姣忓皬鏃?converge 鐨?auto_update 妫€鏌ュ悓姝?
  鏀逛负 `!= Some(true)` 鎷︽埅
- 鎭㈠鑷姩鏇存柊锛歝onfig.toml 鍐?`[cli] auto_update = true`

## 鍏湡琛ヤ竵锛堝疄楠屾€у伐鍏疯緭鍑哄帇缂╋級

> 榛樿鍏筹紱`[tool_output_compression] enabled = true` 鎵嶆敼琛屼负銆傛湭寮曞叆
> `only-cc-lite` git 渚濊禆锛岀瓥鐣ユ寜 headroom 绠€鍖栵細json / logs / search / diff /
> generic head+tail銆俙exit_code`/`stderr` 涓?bash `exit: N` 澶存棤鎹熴€?
> 浼氳瘽鍚姩鏃舵妸閰嶇疆閽夎繘 SharedResources锛涘凡鍐欏叆瀵硅瘽鐨?tool_result 姘镐笉鍥炲啓锛?
> 鎻愮ず缂撳瓨鍓嶇紑鍦ㄦ暣娈典細璇濆唴瀛楄妭绋冲畾銆傛敼閰嶇疆鍙奖鍝嶄笅涓€涓柊浼氳瘽銆?

### 浜屾湡鏂瑰悜瀹氱锛?026-09-17锛屾憳瑕侊紱鍏ㄦ枃瑙?`tool-output-compression-plan.md`銆屼簩鏈熸柟鍚戙€嶏級

鐩爣鍗囩骇涓?*浣庢崯/鏃犳崯**锛岀粍鍚堜负涓夌骇鐎戝竷锛氭棤鎹熷眰锛堥噸澶嶈鎶樺彔 xN銆丣SON 绮剧‘鍘婚噸銆?
search 鏍囬鍖栦繚鍏ㄨ銆乨iff index 鍓ョ銆丄NSI 鍓ョ锛涘叏閮ㄥ彲閫?+ 寰€杩旇嚜鏍￠獙澶辫触閫€鍥?
鍘熸枃锛夆啋 浣庢崯灞傦紙鏃ュ織閲嶈鎬ф墦鍒嗛檺棰勭畻銆丣SON 鑶濈偣 adaptive-k锛夆啋 鏈夋崯鍏滃簳
锛坓eneric head/tail锛屾柊澧?`max_lossy_ratio = 0.25` 涓婇檺锛屾埅鏂繀鍐?CCR marker锛夈€?

浜嬪疄渚濇嵁锛堟湰鍦版秷铻?`mech_ablation_report` 娴嬭瘯 + 鍥涘绀惧尯椋庤瘎锛夛細

- 鏈湴娑堣瀺锛氶噸澶嶅瀷 JSON 涓€鏈熷熀绾跨渷 92.7% 浣?*涓腑闂村敮涓€椤?*锛岀簿纭幓閲?91.3% 闆朵涪澶憋紱
  鏃ュ織妯℃澘鎶樺彔鐪?93.1% vs 鍩虹嚎 78.8% 涓斿叧閿鍏ㄤ繚鐣欙紱search 姣忔枃浠?3 鏉′笂闄?
  涓㈠熬閮ㄥ懡涓紙涓€鏈熼殣钘忔崯澶憋級锛涘敮涓€鍨嬪唴瀹?浣庢崯=浣庣渷"鏄墿鐞嗘瀬闄愶紙鍏ㄤ繚鐣欎粎鐪?23-25%锛夈€?
- headroom 涓俊鎭姇璇夛細#3545 search 琛岀啍鎺?鈫?琛屽彿鈫斿唴瀹瑰亣閰嶅锛?3580 浠ｇ爜琚?ML
  閫氶亾鍒犺瘝锛?3590 灏忕粨鏋勫寲杈撳嚭鍘嬫畫锛?3625 鎴柇鏈啓 marker锛?3544/#3560 鏈?marker
  鏃?retrieve锛?3587 1886 璇锋眰闆舵妫€绱紙marker 鎴愭湰鐧戒粯锛夈€傛棤鎹熸瑙ｅ湪瀹冪殑
  `lossless_compaction.py`锛堝彲閫?+ 鑷牎楠岋級銆?
- 瀹炴祴缁忔祹璐︼細閲嶅啓鍘嗗彶 鈫?cache bust 123 vs 14锛屽噣鐪?鈮?锛坆randonbarker.me 瀵圭収
  瀹為獙锛夛紱headroom 鑷姤 savings ~1.9x 楂樹及锛?stats 鏁板瓧鍙綋鐩稿鎸囨爣锛夛紱
  tsheadroom 淇濆畧妗ｅ疄娴嬩粎 ~40%锛坴s 瀹ｄ紶 60-95%锛夈€?
- DCP锛堟ā鍨嬩富鍔ㄥ帇缂╋級涓嶉噰鐢細鎽樿鑶ㄨ儉鍙嶇儳 738k token锛?573锛夈€侀潤榛樹涪鏁版嵁锛?534锛夈€?
  鍘熷湴鏇挎崲鐮?cache锛?604锛夈€佷繚鎶ょ櫧鍚嶅崟 Windows 璺緞鍒嗛殧绗︿粠鏈尮閰嶏紙#592锛夈€?
- context-mode锛堜簨鍓嶆矙绠憋級涓嶉噰鐢細MCP 鐩插尯 + FTS5 鍙洖渚濊禆妯″瀷鍐欏鑴氭湰 +
  evict 鎺掑簭 bug锛涙湰 fork 璇ュ満鏅敱 rtk hook 瑕嗙洊銆?

### xai-grok-tools
- 鏂版ā鍧?`implementations/output_compression/`锛氭娴嬨€佸帇缂┿€丆CR 鏂囦欢搴撱€?
  `expand_output` 宸ュ叿銆佽繘绋嬬骇 ledger
- `registry/types.rs` `finalize_output`锛歱rompt 鏂囨湰鍘嬬缉锛堟彁閱掍箣鍓嶏級
- `ToolRegistryBuilder::new` 娉ㄥ唽 `expand_output`锛坉ispatch锛夛紱骞垮憡闈㈢敱
  AgentBuilder 鍦?CCR 寮€鍚椂娉ㄥ叆

### xai-grok-shell
- `Config.tool_output_compression`锛坰erde default锛宻truct 灏鹃儴锛?
- `resolve_runtime_fields` 璋冪敤 `set_runtime`

### xai-grok-agent
- `builder.rs`锛欳CR 寮€鍚椂鎶?`expand_output` 娉ㄥ叆 toolset

### xai-grok-pager
- `/stats` 鏂滄潬鍛戒护 + `grok stats` 娈碉細姝ｅ悜 saved tokens/%锛岃礋鍚?expanded +
  retrieve 鍥炵亴 tokens锛孋CR 棰濆 I/O ops/ms
- `docs/user-guide/29-local-enhancements.md` 閰嶇疆璇存槑

## 涓冩湡琛ヤ竵锛?026-09-18锛氱晫闈㈡枃妗堜腑鏂囧寲 P0 浜や簰蹇呯粡锛?

> 鏂规瑙?`docs-local/ui-i18n-plan.md`锛圥0 = 姣忎釜浼氳瘽閮芥挒涓婄殑浜や簰璺緞锛? 涓鍥撅級銆?
> 缈昏瘧琛?280 鈫?449 缁勶紱鏂藉伐瑙勭害涓庢湳璇〃浠ユ柟妗堟枃妗ｄ负鍑嗭紙鐘舵€佸瓨鑻辨枃閿覆鏌撳嚭鍙ｇ炕璇戙€?
> 姣旇緝/鍖归厤閿笉鍖呯炕璇戙€佹祴璇曟瀯寤烘煡琛ㄦ梺璺笉鍙橈級銆?

### xai-grok-pager锛圥0锛氬懡浠ら潰鏉?鍚姩灞?闅愮妯箙/浼氳瘽閫夋嫨/寮曞閲囬泦/鏉冮檺路鎻愰棶路璁″垝瀹℃壒锛?
- `src/slash/i18n.rs`锛氱炕璇戣〃杩藉姞 P0 娈?169 缁勶紙鍛戒护闈㈡澘鏉＄洰鍚嶄笌鎸夐挳銆佸惎鍔ㄥ睆淇′换纭/
  璁よ瘉娴佺▼/鑿滃崟/鐩稿鏃堕棿璇嶆棌銆侀殣绉佹í骞呭垎娈垫枃妗堛€佷細璇濋€夋嫨杩囨护寰界珷涓庡姞杞藉ご銆佸紩瀵奸噰闆?
  鏍囩涓?URL 鏍￠獙閿欒銆佹潈闄愭ā寮忕紪杈戦瑙堜笌椤佃剼銆佹彁闂崰浣嶇銆佽鍒掑鎵圭姸鎬佹爣绛句笌绌鸿鍒?
  鍗犱綅娈电瓑锛?
- `src/views/modal.rs`锛?6 鏉″懡浠ら潰鏉挎潯鐩湪 `default_palette_entries` 鏋勯€犲 tr锛堟覆鏌?
  鍑哄彛鍦?app/modals.rs锛屽睘鍚庣画鎵规锛涘凡鏍稿疄 label 鏃犳瘮杈冪偣锛屼腑鏂囨ā寮忎笅鎸?shortcut 鍒?
  浠嶅彲鑻辨枃妫€绱級锛涙寜閽?label()/闈㈡澘鏍囬闂彞/reset 纭鎷嗙墖娈?docs 閫夋嫨鍣ㄤ笌鏌ョ湅鍣ㄩ〉鑴?
  鍥句緥绛夌害 14 缁?
- `src/views/welcome/mod.rs`锛歵rust 纭閫愯鎴愰敭銆佽璇佹祦绋嬪父閲忥紙AUTH_HEADER 绛夋覆鏌撳
  鏌ヨ〃锛夈€佽彍鍗曢」銆乊es/No 纭銆乬ate 灞忋€佹洿鏂伴€氱煡妯℃澘銆佺浉瀵规椂闂存棌锛?just now" 淇濈暀
  鑻辨枃閿紝goal_detail.rs 鐨?`ago == "just now"` 姣旇緝涓嶅彈褰卞搷锛夛紱3 澶勫懡涓煩褰?鎹㈣
  浼扮畻 `.len()` 鈫?`.width()`锛堟祴璇曟瀯寤鸿蛋鑻辨枃鏃佽矾锛孉SCII 瀹藉害涓嶅彉锛屾柇瑷€涓嶅彈褰卞搷锛?
- `src/views/privacy_banner.rs`锛氭爣棰?璇存槑娈垫暣娈垫垚閿紱LEGAL 閾炬帴閫愭鎴愰敭淇濆垎娈垫暟锛?
  鐑尯瀹藉害鏀规寜璇戞枃 `shown.width()` 璁★紙涓夊彉浣撲腑鏂囨覆鏌撳搴﹀潎 鈮?鑻辨枃锛岄€夋。涓嶅彉閲忎繚鎸侊級
- `src/views/session_picker.rs`锛歚SourceFilter::label()` 鍏釜杩囨护寰界珷銆?(no prompt)"/
  "(no summary)"銆佸姞杞藉ご锛坰pinner 鏀?`"{} {}"` 鎷兼帴锛岃嫳鏂囪緭鍑洪€愬瓧涓嶅彉锛夈€乭idden 澶栭儴
  浼氳瘽璁℃暟妯℃澘鏁撮敭锛沗session_picker_surface.rs` 闆舵敼鍔紙"Open session" 鏍囬缁?
  modal_window 娓叉煋鍑哄彛 tr_str 鍛戒腑锛屽浘渚?nav/select/close/search 閿凡鍦ㄨ〃锛?
- `src/views/elicitation_view/{render,state}.rs`锛氭爣绛?鎸夐挳/绛夊緟/婊氬姩鏍囪 render 澶?
  tr锛? 涓爣棰樻ā鏉夸笌 4 鏉?URL 鏍￠獙閿欒鏋勯€犲 tr 鍚?replace 鍗犱綅锛堥敭鍚?{} 鍗犱綅绗︼級
- `src/views/permission_view.rs` + `question_view.rs` + `plan_approval_view.rs`锛氭ā寮?
  缂栬緫 5 鎬侀瑙堣/椤佃剼鍔ㄤ綔璇?`"all tools from {}"` 鎷嗙墖娈碉紱鎻愰棶鍗犱綅绗︿笌鎴柇鎻愮ず
  锛坬uestion_view:834 "Other" 鏄?ACP 绾夸笂鍗忚涓诧紝涓嶈瘧锛涘彲瑙佽鏍囩鍦?dashboard/peek.rs
  灞?P3锛夛紱`plan_approval_status_label` 绾睍绀哄嚭鍙?tr
- `src/app/agent_view/plan.rs`锛氱┖璁″垝鍗犱綅娈垫覆鏌撳嚭鍙?`tr_str(EMPTY_PLAN_PLACEHOLDER)`
  锛堝父閲忔湰浣撲繚鎸佽嫳鏂囷紝trim 鍒ょ┖閫昏緫涓嶅姩锛?
- 閬楃暀锛堝悗缁壒娆″鐞嗭級锛歴ession_picker 灞曞紑鍗″瓧娈垫爣绛?ID/CWD/Created/鈥?鍥?picker.rs
  鐢?`{:<12}` 瀛楃琛ラ綈 + `.len()` 甯冨眬鏆備笉璇戯紝闇€鍏堟妸璇ュ鏀?unicode_width锛涘懡浠ら潰鏉?
  娓叉煋鍑哄彛 app/modals.rs銆乸eek.rs "Other" 琛屾爣绛惧湪 P3

## 鍏湡琛ヤ竵锛?026-09-18锛歳elease 鏋勫缓鍛婅娓呯悊锛?

> v1.0.31 win/linux release 鏋勫缓鏃ュ織涓殑 rustc 鍛婅娓呴浂锛屾棤琛屼负鍙樻洿锛涗笂娓稿悓姝ュ啿鎺夊悗鎸夋湰鏉￠噸鏀俱€?

### xai-grok-shared
- `src/clipboard.rs` `get_text`/`get_image`锛歚arboard_error` 鐢?鍏堝垵濮嬪寲 None 鍐?match 璧嬪€?
  鏀逛负鎸?match 鑷傜洿鎺ヤ骇鍑轰笉鍙彉缁戝畾锛圵indows 涓?Ok(None) 鎻愬墠杩斿洖瀵艰嚧鍒濆鍖栧€兼案涓嶈璇伙紝
  `unused_assignments` 鍛婅 脳2锛夛紱閿欒鍦ㄥ熬閮ㄧ殑 `if let Some(error)` 缁熶竴涓婃姏锛岃涔変笉鍙?

### xai-grok-pager-render
- `src/terminal/probe.rs`锛歚use std::time::Duration` 鍔?`#[cfg(unix)]`锛堜粎 unix 闂ㄦ帶鐨?
  `LATE_REPLY_GRACE`/`read_tty_reply` 浣跨敤锛學indows 渚?unused import锛?

### xai-grok-hooks
- `src/runner/mod.rs`锛歚gate_outcome`锛堟枃妗ｆ敞鏄?legacy test surface锛夊姞 `#[cfg(test)]`锛?
  release 鏋勫缓涓嶅啀缂栬瘧锛坲nused fn 鍛婅锛?
- `src/runner/command.rs`锛歚gate_outcome` 瀵煎叆鎷嗗垎涓?`#[cfg(test)] use super::gate_outcome;`
- `src/runner/command.rs` 娴嬭瘯妯″潡锛歚make_scoped_ctx` 鍔?`#[cfg(unix)]`锛堜粎 unix 闂ㄦ帶鐨?
  杩涚▼缁勬祴璇曚娇鐢紱Windows 娴嬭瘯鏋勫缓 dead_code 鍛婅锛屾瀯寤烘棩蹇椾笉鏄剧ず浣?`--all-targets` 鍙锛?

## 涔濇湡琛ヤ竵锛?026-09-18锛氱晫闈㈡枃妗堜腑鏂囧寲 P1 甯哥敤寮圭獥涓庡府鍔╋級

> 鏂规瑙?`docs-local/ui-i18n-plan.md`锛圥1 = 甯哥敤寮圭獥涓庡府鍔╋紝5 涓ā鍧楋級銆?
> 缈昏瘧琛?449 鈫?917 缁勶紱鏂藉伐瑙勭害涓庢湳璇〃浠ユ柟妗堟枃妗ｄ负鍑嗐€?

### xai-grok-pager锛圥1锛氳缃脊绐?蹇嵎閿€熸煡琛?鐢ㄩ噺寮圭獥/MCP 寮圭獥/鏁欑▼锛?
- `src/slash/i18n.rs`锛氱炕璇戣〃杩藉姞 P1 娈?430 缁勶紝鍒嗚妭涓庝唬鐮佹敞閲婁竴涓€瀵瑰簲锛?
  settings 瀛楅潰閲?椤佃剼 rest 閿?鍒嗙粍鏍囬銆乺egistry meta.label+description銆佹灇涓?
  display+description锛堝惈 STT 璇█鍚嶄腑鏂囧寲锛夈€乻hortcuts 鍒嗙被/椤佃剼/浼/澶氳 long_help
  甯搁噺/ActionRegistry short_help+long_help銆乽sage 鏍囩椤?椤佃剼 rest 閿?allowance+浼氳瘽
  淇℃伅瀛楁銆乵cps 鍒嗙粍妯℃澘+鐘舵€佸窘绔犮€乼utorial 寮曞璇?涓婚 title/blurb+go_deeper 鎸囧崡椤垫爣棰?
- `src/views/settings_modal/render.rs`锛氶潰鍖呭睉/鍒嗙粍鏍囬/琛屾爣绛?琛屽€煎窘绔?灞曞紑鎻忚堪涓庨攣瀹?
  鍘熷洜/Tip/杩囨护绌烘€侊紙`tr("No matches for ")` 璇戞枃鑷韩鍙備笌瀹藉害璁＄畻锛屽竷灞€涓庣粯鍒跺悓婧愶級/
  缂栬緫鍣ㄥ崰浣嶇涓庢牎楠岄敊璇紙`Unknown model: "{}"` 妯℃澘閿?strip 鍓嶅悗缂€锛?鏋氫妇閫夋嫨鍣?
  display+description 娓叉煋鍑哄彛缁熶竴 tr/tr_str锛涗笁澶?`row_layout` 鏍囩瀹藉害鍚屾浼犺瘧鏂?
- `src/views/shortcuts_help.rs`锛欰ctionRegistry 娓叉煋閾捐矾鏈鎺ョ嚎鈥斺€擿entry_display`
  锛坔int 璇存槑+鍒嗙被鏍囬锛夈€乣CheatsheetRows::build`锛堟姌鍙犳爣棰?鍐呰仈甯姪锛夈€乣render_detail`
  锛堣鎯呴〉 title/body 娓叉煋鍑哄彛 tr_str锛宻tate 瀛樿嫳鏂囦笉鍙橈級銆乣render_detail_body` 鍙樼伆
  娉ㄨ銆侀〉鑴氫笌 3 澶勫脊绐楁爣棰橈紱鎼滅储杩囨护 `filter_entries` 浠嶆寜鑻辨枃鍖归厤锛堜腑鏂囨ā寮忎笅鐢?
  鑻辨枃璇嶆悳绱紝濡傞渶涓枃鎼滅储闇€鍗曠嫭缈昏瘧鍖归厤灞傦級锛沗src/app/modals.rs` 浠?2 澶?
  "Keyboard Shortcuts" 鏍囬鎺ョ嚎
- `src/views/usage_modal.rs`锛氫笁涓爣绛鹃〉鏍囬锛坢odal_window 娓叉煋鍑哄彛鏌ヨ〃锛夈€侀敊璇?绌烘€?
  鍔犺浇涓€乤llowance 鍖猴紙`Usage: ${used} / ${cap} per month` 鍛藉悕鍗犱綅绗?replace锛夈€?
  浼氳瘽淇℃伅瀛楁鏍囩灞忔樉鍑哄彛 tr锛涘壀璐存澘澶嶅埗涓叉媶寮€淇濇寔鑻辨枃锛堝鍒跺唴瀹瑰亸鏁版嵁锛屼笖 dispatch
  娴嬭瘯瀵瑰鍒舵枃鏈湁鑻辨枃鏂█锛?
- `src/views/mcps_modal.rs`锛氬垎缁勬爣棰樻ā鏉挎暣閿紙`"Managed by grok.com ({})"` replace
  璁℃暟锛涙彃浠跺垎缁?`"Plugin: "` 鍓嶇紑閿?鍔ㄦ€佸悕鐣?format! 鍙傛暟锛夈€丮anaged 璇存槑琛屻€? 涓姸鎬?
  寰界珷 label()锛堝凡鏍稿疄鍏ㄩ儴娑堣垂鐐逛负灞曠ず锛屾棤姣旇緝閿級
- `src/views/tutorial.rs`锛欼NTRO_LINES 娓叉煋鍑哄彛閫愭潯 tr銆佸垪琛ㄨ title/blurb tr銆佷袱椤甸〉鑴?
  锛坄{}/{} explored` 鍙屽崰浣?replacen锛夛紱`tutorial_docs.rs` 闆舵敼鍔ㄢ€斺€攖itle/blurb 鏄?
  static 涓嶈繘 state 涔熶笉琚瘮杈冿紝寮圭獥鏍囬璧?docs 鏌ョ湅鍣ㄤ腑澶?tr_str锛沢o_deeper 鐨?
  `find_doc(title)` 绱㈠紩閿繚鎸佽嫳鏂?
- `src/views/modal.rs`锛歳eset 纭鎻掑€艰ˉ `tr_str(&meta.label)` 涓庨粯璁ゅ€煎睍绀?tr_str
  锛堜笌 settings 寮圭獥琛屾爣绛捐瘧鏂囧榻愶級
- 涓婚涓撳悕锛圙rok Night/Tokyo Night 绛夛級銆?ZDR"銆佹ā鍨嬪悕涓嶈瘧锛堜笌 `/theme <name>` 鐢ㄦ硶
  涓€鑷达級锛泂ettings 椤佃剼鍥句緥 "type to filter" 璇戞枃鍛?"type 浠ヨ繃婊?锛坰hortcut_label_i18n
  鍥哄畾淇濈暀閿綅 token锛屽睘鏈哄埗闄愬埗锛屽悗缁闇€鏁村彞鎴愰敭瑕佹敼 modal_window 娓叉煋鍑芥暟锛?

## 鍗佹湡琛ヤ竵锛?026-09-18锛氱晫闈㈡枃妗堜腑鏂囧寲 P2 闆嗘垚绠＄悊寮圭獥锛?

> 鏂规瑙?`docs-local/ui-i18n-plan.md`锛圥2 = 闆嗘垚绠＄悊寮圭獥锛? 涓ā鍧楋級銆?
> 缈昏瘧琛?917 鈫?1138 缁勶紙+221锛夛紱鏂藉伐瑙勭害涓庢湳璇〃浠ユ柟妗堟枃妗ｄ负鍑嗐€?

### xai-grok-pager锛圥2锛氭墿灞?璁板繂/鍙嶉/瀵煎叆 Claude 浜斿脊绐楋級

- `src/slash/i18n.rs`锛氱炕璇戣〃杩藉姞 P2 娈靛叡 221 缁勶紝鍒嗚妭涓庝唬鐮佹敞閲婁竴涓€瀵瑰簲锛?
  import_claude锛堟爣棰?绫诲瀷鍒嗙粍澶?鑼冨洿澶?Enter 纭妯℃澘/椤佃剼 rest 閿級銆乵emory锛堣妭澶?鍗犱綅绗?
  绌烘€?椤佃剼/鐩稿鏃堕棿锛夈€乫eedback锛堢Щ鍑洪€氱煡/trace 闂彞/鏍囩琛?绌烘€?瀛樺偍闀垮彞/taxonomy 鏋氫妇
  label/鏍囬鏍囩椤碉級銆乪xtensions锛堝垎缁勫ご/寰界珷/璁℃暟妯℃澘/琛ㄥ崟/椤佃剼鍔ㄤ綔璇嶏級銆乵odals.rs 渚?
  锛堢‘璁ら棶鍙ュ墠缂€/鍚庣紑閿笌闈欐€佹彁绀猴級銆傚悎骞舵椂 "Name"/" cancel"/"Hooks"/"navigate"/"toggle"/
  "cancel"/"search"/"Import Claude settings" 绛変笌鏃㈡湁鏉＄洰鍚岄敭鍚岃瘧锛屾寜鏃㈡湁鏉＄洰鍘婚噸锛?
  鏌ラ噸鑴氭湰鎸?translations() 鍏ㄨ〃瑙ｆ瀽鏂█鏃犻噸澶嶉敭锛堜節鏈熺殑涓存椂鑴氭湰宸叉竻鐞嗭紝閲嶆斁鏃舵寜鏈潯
  鎻忚堪閲嶅缓鍗冲彲锛?
- `src/views/extensions_modal.rs` + `extensions_modal/workflows_picker_rows.rs`锛?
  6 涓爣绛鹃〉鍚嶆覆鏌撳嚭鍙?tr锛涘垎缁勫ご鏂板 `tr_group_label`锛坄Plugin: {name}`/`Custom: {path}`
  杩愯鏃舵嫾鎺ヤ覆鎸夋棦鏈?`Plugin: ` 鍓嶇紑閿媶鍒嗭紝鍏朵綑鏁翠覆 tr_str锛涘垎缁勮嫳鏂囨爣绛炬槸鎶樺彔 state 閿?
  淇濇寔鑻辨枃锛夛紱璁℃暟妯℃澘鏁撮敭 + replace锛坄{n} plugins`/`{n} skills`/`{n} tools ({m} enabled)`
  绛夛紝鍗曞鏁颁腑鏂囧悎骞讹級锛沗post_select_row_hint` 鏁村彞妯℃澘閿?+ `{noun}`(tr_str)/`{verb}`(tr)
  娉ㄥ叆锛坉isable/enable 鎴愬锛夛紱寰界珷 [policy]/[disabled]/[installed]/[error]/[update available]锛?
  灞曞紑瀛楁鏍囩锛沗Error: {msg}` 鎷嗕负 `format!("{}: {msg}", tr("Error"))`锛沵odal_message
  娓叉煋鍑哄彛 tr_str锛堢被鍨?`(&str, Color)` 鈫?`(String, Color)`锛夛紱result_notice/pending 寰界珷/
  琛ㄥ崟鏍囩涓庡崰浣嶇娓叉煋鍑哄彛 tr_str锛沬nstall_status 灞忔樉鍊艰ˉ `not_installed`/`update_available`
- `src/views/memory_modal.rs`锛氳妭澶?Global/Workspace/Sessions 瀛?state 鑻辨枃锛坈ompute_filtered
  鍋?contains 杩囨护锛夆啋 娓叉煋鍑哄彛 tr_str锛涢〉鑴?13 鏉?tr锛沠ormat_modified 鐩稿鏃堕棿妯℃澘閿?
  锛坄{mins}m ago` 澶嶇敤鏃㈡湁閿紝`{hours}h`/`{days}d` 鏂板锛夛紱鍒犻櫎纭琛屽唴鎻愮ず閿惈鍓嶅绌烘牸锛?
  瀵归綈瀹藉害 `len()` 鈫?`width()`锛堣嫳鏂囪涓轰笉鍙橈紝涓枃璇戞枃淇鍙冲榻愶級
- `src/views/feedback_modal/{mod,render,enum_picker}.rs`锛坉rafts.rs 闆舵敼鍔紝鍏跺瓧绗︿覆鍏ㄩ儴
  鏄瓨 state 鐨勮嫳鏂囬敭锛夛細7 鏉＄Щ鍑洪€氱煡 notice() tr锛泃race 閫夐」/纭闂彞/绌烘€?鍒犻櫎纭/
  composer 鍗犱綅绗?tr锛沞rror 娓叉煋鍑哄彛 3 澶?tr_str 瑕嗙洊 drafts.rs 鍏ㄩ儴瀛樺偍閿紙澶氭潯澶氳闀垮彞
  鏁存鎴愰敭锛屼笌婧愮爜閫愬瓧绗︽牳瀵瑰惈鍒嗗彿涓?U+2026锛夛紱enum_picker 琛?`tr(labels[variant])`锛?
  type-to-filter 杩囨护姣旇緝閿繚鎸佽嫳鏂囷紱xai_grok_feedback taxonomy 鏋氫妇 label 娓叉煋鍑哄彛鏌ヨ〃
  骞惰ˉ 25 閿紙Bug/Shell 淇濈暀鑻辨枃涓嶅叆琛級
- `src/views/import_claude_modal.rs`锛氱被鍨嬪垎缁勫ご `tr(kind.label())`锛汫lobal 鑼冨洿澶存暣閿瓨
  state 鈫?render_header_line 鍑哄彛 tr_str锛汸roject 鑼冨洿澶?`tr("Project  ")` 鍓嶇紑閿繚瀹斤紱
  `"Enter import {}"` 妯℃澘閿?+ replace锛坰hortcut_label_i18n 鍐嶆媶鏃惰瘧鏂囬€忎紶锛夛紱椤佃剼
  navigate/toggle/fold/all/none/cancel/search 璧?rest 閿満鍒讹紙閮ㄥ垎 P0/P1 宸插叆琛級
- `src/app/agent_view/modals.rs`锛歠eedback 绉诲嚭閫氱煡閰嶅涓ゅ彞 tr锛堜笌 notice() 鍚屼竴鏉＄郴缁?
  閫氱煡锛夛紱extensions 纭闂彞甯﹀姩鎬佸悕鐨勭敤鍓嶇紑閿紙`format!("{}\"{name}\"?",
  tr("Remove MCP server "))` 寮忥紝鑻辨枃杈撳嚭涓庡師 format! 閫愬瓧涓€鑷达紝娴嬭瘯鏋勫缓鏃佽矾涓嶅彈褰卞搷锛夛紱
  pending_action 闈欐€佸€硷紙Reloading.../Processing.../adding.../Adding source.../
  Uninstalling.../Installing...锛夊彧琛ヨ〃锛屾覆鏌撳嚭鍙ｅ凡鎺?tr_str

### 宸茬煡浣欑暀

- `Authenticating {server}...`锛坢odals.rs 瀛樺偍鏈熸彃鍊硷級鏃犳覆鏌撲晶妯℃澘鍙媶锛屼腑鏂囨ā寮忔樉绀鸿嫳鏂囷紱
- "Dropped {n} invalid image(s)." 閲囩敤瀛樺偍鏈熸ā鏉块敭 + replace锛堟覆鏌撲晶鎷夸笉鍒拌鏁帮級锛?lang
  鍒囨崲涓嶈拷婧凡瀛樻枃妗堬紱
- tests 鏂█鐨?state 鍊煎叏閮ㄤ繚鎸佽嫳鏂囷紙娴嬭瘯鏋勫缓鏌ヨ〃鏁翠綋鏃佽矾锛夛紝tests 妯″潡闆舵敼鍔ㄣ€?

## 鍗佷竴鏈熻ˉ涓侊紙2026-09-18锛氱晫闈㈡枃妗堜腑鏂囧寲 P3 浠〃鐩樹笌浠ｇ悊闈㈡澘锛?

> 鏂规瑙?`docs-local/ui-i18n-plan.md`锛圥3 = 浠〃鐩樹笌浠ｇ悊闈㈡澘锛? 涓ā鍧楋級銆?
> 缈昏瘧琛?1138 鈫?1338 缁勶紙+200锛夛紱鏂藉伐瑙勭害涓庢湳璇〃浠ユ柟妗堟枃妗ｄ负鍑嗐€?
> 鍚岄敭寮傝瘧浠茶锛欶ailed鈫掑け璐ワ紙dashboard/goal_detail/agent_status 涓夋柟缁熶竴锛夈€?
> Paused (error)鈫掑凡鏆傚仠锛堝嚭閿欙級锛沜onfirm delete 涓?P0 鏃㈡湁閿挒閿垹閲嶃€?

### xai-grok-pager锛圥3锛歞ashboard 鐢熶骇鍖?/ tasks_pane / agent 椤佃剼+agent_status / goal_detail / workflows锛?

- `src/slash/i18n.rs`锛氱炕璇戣〃杩藉姞 P3 娈?200 缁勶紝鍒嗚妭涓庝唬鐮佹敞閲婁竴涓€瀵瑰簲锛?
  dashboard chrome/row/render/peek銆乬oal_detail 鐘舵€佷笌瀛楁鍓嶇紑銆乤gent 椤佃剼 hint 璇嶆棌銆?
  agent_status chip銆亀orkflows 妯℃澘涓庣姸鎬佽鏄庛€乼asks_pane 璋冨害鍚庣紑
- `src/views/dashboard/chrome.rs`锛歝hip 缁樺埗渚?tr(label)锛坔it-test id 淇濈暀鑻辨枃锛夈€?
  Choose銆? New Agent( in Worktree)銆乄orktree/Disable Worktree 鎸夐挳
- `src/views/dashboard/row.rs`锛氶€愬抚閲嶅缓澶勬垚閿紙`{tools} tools 路 {toks} tok 路
  {turns} turns`銆乣鈥?{} more` 澶嶇敤鏃㈡湁閿級锛涙暣涓茬姸鎬佽瘝锛圵orking/Awaiting your
  input/Loading鈥?Pending: question锛変粛瀛樿嫳鏂囩敱 render.rs 鍑哄彛 tr_str锛?
  AgentCommand::display_name()锛坅pp/agent.rs 浜斿€硷級缁勫悎澶?tr
- `src/views/dashboard/render.rs`锛?*鏂规鏂囨。"鐢熶骇鍖轰粎 1鈥?103 琛?鏈夎**鈥斺€斿疄闄?
  `#[cfg(test)] mod tests` 鍦?3640 琛岃捣锛坮ender_tests.rs锛夛紝1104/1950 鍙槸涓や釜 6 琛?
  cfg(test) 杈呭姪鍑芥暟锛涙湰娆℃寜鐪熷疄杈圭晫鎺ョ嚎鏁翠釜鐢熶骇鍖猴細banner 澶嶆暟涓枃鍚堝苟銆佺┖鎬?杩囨护銆?
  鍒嗙粍澶达紙Pinned + `tr(rs.group_label())`锛夈€両dle overflow銆佷綅缃€夋嫨鍣ㄣ€佹ā寮忔棗鏍?
  锛坧lan/auto/always-approve锛夈€佹悳绱笌娲惧彂鍗犱綅绗︺€乣rename: ` 鍓嶇紑鏀舵暃 rename_prefix()
  淇濊瘉缁樺埗涓庡搴﹁绠楀悓婧愩€侀〉鑴氬叏閮?hint锛堝惈 state.rs 鐨?label()/confirmation_label()/
  group_label()/focused_action_label() 璋冪敤鐐?tr锛夈€佽鐩栧眰鍏滃簳涓?[Dashboard]
- `src/views/dashboard/peek.rs`锛歳esponse_type 瀛樿嫳鏂囨瘮杈冮敭锛坄== "Working"` 涓嶅姩锛夆啋
  灞曠ず鍑哄彛 tr_str锛圱hinking/Thought/Response/Read/Edit/鈥?17 璇嶏級锛涢棶棰橀€夐」 label
  鍑哄彛 tr_str锛沚lock_short_text 9 涓嫭娉紙(thinking)/(tool call)/鈥︼級鎴愰敭
- `src/views/dashboard/peek_tail.rs`锛氭牳鏌ョ敓浜ц矾寰勬棤璇存槑鎬ф枃妗堬紝闆舵敼鍔?
- `src/views/goal_detail.rs`锛氱姸鎬佽/瀛楁琛屽墠缂€锛堝熬闅忕┖鏍兼槸閿殑涓€閮ㄥ垎锛?鍒嗚妭澶?
  浜嬩欢鍊间晶/verdict 鏍囩/椤佃剼鎺ョ嚎锛涗簨浠?match 閿紙goal_created 绛夛級涓?`d != "user"`
  姣旇緝閿笉鍔紱鐩稿鏃堕棿鏃忓鐢?{mins}m/{hours}h/{days}d ago/just now 鏃㈡湁閿紝鏂板
  {months}mo/{years}y ago锛沘ctive_phase_label 宸插湪 agent_status 鍑哄彛缈昏瘧锛屾湰鏂囦欢
  浠?tr_str 閫忎紶鍏滃簳锛堟柦宸ユ湡涓存椂 tr_phase_text 鍙屼繚闄╁凡绠€鍖栫Щ闄わ級
- `src/views/agent_status.rs`锛歡oal_phase_label 5 涓?pause 鍒嗘敮 tr(pause_label()) +
  Failed/Interrupted/Budget/Done锛沘ctive_phase_label 鐨?`Verifying ({})` 妯℃澘閿?+
  replace锛沢oal_status_line 璁℃暟鏁撮敭 replacen锛坽} tokens/{}/{} tokens锛夛紱chip_name
  = tr("Goal")銆俻ause_label()锛坅pp/agent.rs:364锛夋牳瀹炵函灞曠ず鏃犳瘮杈冩秷璐癸紝鎺ョ嚎瀹夊叏
- `src/views/agent.rs`锛?*闆舵敼鍔?*鈥斺€旈〉鑴?hint 闆嗕腑缈昏瘧鍑哄彛宸插湪 shortcuts_bar.rs
  锛圥0 鎺ョ嚎锛夛紝鏈鍙ˉ 10 缁勮〃閿紙hide done/show done/reorder/page/queue/newline/
  accept suggestion/next/prev/turn/expand thinking锛夛紱HintItem label 瀛樿嫳鏂囩粡
  ShortcutsBar::render 缁熶竴鏌ヨ〃锛屽氨鍦板啀鍖呬細鍙岄噸缈昏瘧
- `src/views/workflows.rs`锛氶〉鑴?9 蹇嵎閿€佹爣棰?Workflow Runs銆佺┖鎬佷袱琛屻€丳hases
  鍒嗚妭澶达紱agents_meta() 鎸?total==1 閫夎嫳鏂囧崟澶嶆暟閿€佷腑鏂囧悎骞讹紱plural() 閲嶆帴鏁撮敭
  妯℃澘锛坣oun 鍙傛暟褰撳墠鎭掍负 "agent"锛宍let _ = noun` 娉ㄩ噴淇濈暀绛惧悕锛夛紱棰勭畻/failed 鐘舵€?
  璇存槑 4 鏉℃暣閿紙鍚?{n} 妯℃澘锛夛紱rail 闃舵鍚嶄笌 roster 鏍囬娓叉煋鍙?tr_str锛堟暟鎹晶
  phase_hits/selected_phase_name/姣旇緝閫昏緫瀛樿嫳鏂囷級锛沗{used} / {total} context` 妯℃澘锛?
  run.status.replace('_'," ") 鍗忚璇嶄繚鐣欒嫳鏂囷紙tasks_pane 渚у鍘讳笅鍒掔嚎鍊?tr_str
  鏌ヨ〃锛屽懡涓?complete/cancelled/interrupted/paused/budget limited/failed 鍒欒瘧锛?
- `src/views/tasks_pane.rs`锛氬垎缁勫ご group.label() tr锛坰earch_text 杩囨护鍖归厤閿粛
  鑻辨枃锛夛紱璋冨害鍚庣紑鏁撮敭鍚墠瀵肩┖鏍硷紙" (next in {})"锛? 澶勶級/" (due now)"/
  " (running)"/" (starting)"锛宭abel 涓?styled 鍚屾簮锛夛紱绌烘€佷笁娈?Span 涓ゆ鎴愰敭锛?
  "Task " 鍓嶇紑澶嶇敤 Task 閿樉寮忚ˉ绌烘牸锛堥伩鍏嶅熬闅忕┖鏍艰繎閲嶅閿級锛沗1 agent`/
  `{n} agents` 澶嶇敤鏃㈡湁妯℃澘閿紱"killing鈥?" 瑕嗙洊灞傛暣閿?

### 浠茶涓庡彇鑸嶏紙P3锛?

- `[worktree:on]`/`[worktree:off]` 寰芥爣涓嶈瘧锛氬搴︽寜 ASCII `len()` 棰勭畻涓?
  `find("on")` 瀛楅潰瀹氫綅楂樹寒閲嶇粯锛岃瘧鏂囧悓鏃剁牬鍧忓搴︿笌瀹氫綅
- dashboard 琛屽勾榫勫垪 format_time_ago锛?2m"/"just now"锛変笉璇戯細`{age:>6}` 鎸夊瓧绗︽暟
  濉厖瀵归綈锛孋JK 鐮村锛泆til 鍑哄彛璺ㄨ鍥惧睘鍚庣画鑼冨洿
- 5s/3m/2h 绱у噾鏃堕暱鍗曚綅淇濈暀锛坅gent_status chip 瀹藉害棰勭畻锛?
- app 灞傚姩鎬佷覆閫忎紶鑻辨枃锛歠ormat_activity_label锛?Running: cargo test"锛夈€?
  format_subagent_label銆乫ormat_context_badge鈥斺€旀暣涓插惈杩愯鏃舵暟鎹棤娉曟垚閿紝
  app 灞傛枃妗堝闇€涓枃鍖栧彟琛岀珛椤?
- tasks_pane 琛?label 鏋勯€犳湡缈昏瘧锛坋ntries 姣忔 sync 閲嶅缓锛夛細涓枃妯″紡涓嬭杩囨护鍖归厤
  涓枃鐗囨锛屼笌 dashboard/render.rs 鍚屾鏃㈠畾鍙栬垗
- 鍗犱綅绗?`<query>` 闅忔鏂囨剰璇戜负 `<鏌ヨ>`锛堟枩鏉犲懡浠ょ敤娉曚覆 [鏂囦欢] 鍏堜緥锛夛紱
  "esc close"锛堝皬鍐欙級涓庢棦鏈?"Esc close" 鍒嗛敭骞跺瓨锛岃瘧鏂囬鏍间竴鑷?
- workflows rail 瀹藉害棰勪及鎸夋湭璇?title 璁＄畻锛屼腑鏂囨爣棰樼暐瀹界敱 truncate_to_width 鍏滃簳锛?
  甯冨眬鏁板鏈姩
- tests 鏂█鐨?state 鍊煎叏閮ㄤ繚鎸佽嫳鏂囷紙娴嬭瘯鏋勫缓鏌ヨ〃鏁翠綋鏃佽矾锛夛紝tests 妯″潡涓?
  *_tests.rs 闆舵敼鍔紱鏌ラ噸鑴氭湰锛坱ranslations() 鍏ㄨ〃瑙ｆ瀽鏂█鏃犻噸澶嶉敭鏃犲悓閿紓璇戯級
  閲嶆斁鏃朵粠鏈枃浠跺崄鏈熸潯鐩弿杩伴噸寤猴紙Python锛孯ust \u{...} 杞箟闇€鑷瑙ｇ爜锛?

## 鍗佷簩鏈熻ˉ涓侊紙2026-09-18锛氱晫闈㈡枃妗堜腑鏂囧寲 P4 浣庢劅鐭ユ壂灏撅紝i18n 璁″垝鍏ㄩ儴瀹屽伐锛?

> 鏂规瑙?`docs-local/ui-i18n-plan.md`锛圥4 = 浣庢劅鐭ユ壂灏撅紝绾?20 涓皬鏂囦欢锛夈€?
> 缈昏瘧琛?1338 鈫?1417 缁勶紙+79锛夛紱鏂藉伐瑙勭害涓庢湳璇〃浠ユ柟妗堟枃妗ｄ负鍑嗐€?
> 鏂藉伐鏂瑰紡锛? 涓苟鍙戝瓙浠ｇ悊鍒嗘壒鎺ョ嚎锛堥潰鏉挎诞灞?/ 寮圭獥 dock 鐘舵€佹潯 / 鍒楄〃鍛戒护灞傦級锛?
> 涓讳細璇濆悎骞堕敭鍊?+ 琛?rewind.rs 涓ゅ娓呭崟澶栨紡缃戙€?

### xai-grok-pager锛圥4锛歫ump/queue/todo/subagent_catalog/btw/location/session_title/
### new_worktree/managed_connectors_wait/hero_box/workspace_mode/dock/credit_bar/
### list_pane/block_viewer/picker/theme/debug/mode_support + rewind 琛ユ紡锛?

- `src/slash/i18n.rs`锛氱炕璇戣〃杩藉姞 P4 娈?79 缁勶紝鍒嗚妭涓庢枃浠朵竴涓€瀵瑰簲锛坖ump/rewind銆?
  queue/todo/subagent_catalog銆乥tw/location/session_title銆乶ew_worktree/
  managed_connectors銆乭ero/workspace_mode銆乨ock銆乧redit_bar銆乴ist_pane銆?
  block_viewer/picker銆乼heme/debug銆乵ode_support 鎷掔粷妯℃澘锛?
- `src/views/jump.rs`锛氭诞灞傛爣棰?"Jump to which turn?"銆?(no preview)" 鍏滃簳 tr
- `src/views/rewind.rs`锛堟竻鍗曞琛ユ紡锛屼笌 jump 鍏辩敤閿級锛氭爣棰?"Rewind to which
  turn?"銆?Loading rewind points..."銆?(no preview)" 涓夊 tr
- `src/views/queue_pane.rs`锛氬琛屽悗缂€ " (+1 line)"/" (+{n} lines)" 鏁撮敭锛堝搴︽寜
  瀹為檯璇戞枃鍔ㄦ€佺畻锛屾棤瀵归綈鐮村潖锛?
- `src/views/todo_pane.rs`锛氱┖鎬?瀹屾垚鎬?4 鏉★紙鍚?{c}/{d} 璁℃暟妯℃澘鏁撮敭 + replace锛?
- `src/views/subagent_catalog_pane.rs`锛氬垎缁勫ご tr_str(owned_name)锛堟瀯閫犳湡鑻辨枃銆?
  鏄剧ず鍑哄彛缈昏瘧锛宻earch_text 杩囨护閿笉鍔級銆佺┖鎬侊紱"Roles" 鏂伴敭锛孭ersonas/Agents
  澶嶇敤鏃㈡湁閿?
- `src/views/btw_overlay.rs`锛歀oading 鎬?"Answering鈥?锛堥敭鍚?U+2026锛?
- `src/views/location.rs`锛歞etached鈫掑垎绂诲ご鎸囬拡銆? (worktree of {repo})" 鍓嶅绌烘牸
  鏁撮敭锛堟祴璇曟瀯寤鸿嫳鏂囪緭鍑洪€愬瓧鑺備笉鍙橈級
- `src/views/session_title.rs`锛氬悎鎴愬洖閫€鏍囬 "session {id}"銆?loading..."銆佺浉瀵?
  鏃堕棿 "now"/"{secs}s ago"锛坽mins}m/{hours}h/{days}d ago 澶嶇敤鏃㈡湁閿彧鎺ョ嚎锛?
- `src/views/new_worktree_dialog.rs`锛氭爣棰?Esc 鎻愮ず/瀛楁鍓嶇紑锛堝熬闅忕┖鏍煎湪閿唴锛?
  " = create   "/" = cancel"锛屽搴﹁绠椾笌娓叉煋鍚屾簮锛坱r(LABEL_PREFIX).width()锛?
- `src/views/managed_connectors_wait.rs`锛歔copied]/[copy the url] 鎸夐挳锛堝懡涓煩褰?
  鐢卞疄闄呯粯鍒朵覆瀹藉害鎺ㄥ锛? 涓よ璇存槑
- `src/views/welcome/hero_box.rs`锛欻ERO_SUBTITLE 鏁存涓€閿紙璇戞枃 57 鍒楃煭浜庡師鏂?
  74 鍒椾笉鎾戠垎鍙虫爮锛夛紱Changelog 澶嶇敤鏃㈡湁閿?
- `src/views/welcome/workspace_mode.rs`锛歴tatus_label() 涓夊垎鏀?tr锛堝敮涓€鐢熶骇鍑哄彛
  鏄姸鎬佹潯缁樺埗锛屾棤姣旇緝娑堣垂锛夛紱"Workspace  " 鍓嶈繘閲忕敱纭紪鐮?11 鏀规樉绀哄搴︼紙鑻辨枃
  绛夊€硷級锛涢€夐」 label() 鍦ㄦ覆鏌撳 tr锛堟柟娉曟湰韬繘鏃ュ織/娴嬭瘯涓嶅姩锛夛紱trailing 鍙冲榻?
  鐢卞瓧鑺?len 鏀规樉绀哄搴︼紙涓枃鎸?3 瀛楄妭浼氶敊浣嶇殑蹇呰浼撮殢淇锛?
- `src/views/dock/mod.rs`锛氬垎缁勮〃澶?tr(section.label())銆?show {n} more" 鏁撮敭銆?
  tab_hint() 涓?kill_label()锛圼stop]锛夊湪鏂规硶鍐呭寘 tr锛堝懡涓煩褰㈢敱缁樺埗涓插搴︽帹瀵硷級锛?
  灏忓啓 subagents/tasks/watchers/queued 鍒嗛敭锛沴ayout.rs 鏃犳枃妗堥浂鏀瑰姩
- `src/views/context_bar.rs`锛?*闆舵敼鍔?*鈥斺€?MAX %" 鏈?PCT_WIDTH=5 鍥哄畾瀹藉害濂戠害
  锛坔over 杩涘害鏉″搴︾敱瀹冨弽鎺?+ 娴嬭瘯鏂█ len==5锛夛紝鏃犵瓑瀹戒腑鏂囩瓑浠风墿锛岃眮鍏?
- `src/views/credit_bar.rs`锛歶sage_label() 鍑哄彛 tr銆佹爣绛惧啋鍙峰墠缂€鍏ㄩ儴閲嶆瀯涓?
  "{}: {}"锛堣嫳鏂囬€愬瓧鑺備笉鍙橈級銆丳AYG 鍙屾彃鍊兼ā鏉?${used}/${cap} 鏁撮敭銆?
  tr_str(&format!("{label} left")) 鍔ㄦ€侀敭涓夋€併€備簨瀹炵粨璁猴細credit_bar_line 绯诲垪
  鐢熶骇鏃犺皟鐢ㄧ偣锛堢姸鎬佹潯涓嶆覆鏌撳畠锛岀湡姝ｅ湪鐢ㄧ殑鏄?usage_warning/format_usage_summary锛夛紝
  閿叆琛ㄥ鐢?
- `src/views/list_pane/render.rs`锛? Copied!" toast锛堥摵鍐欐敼 set_string + 閫愭牸杩樺師
  bg锛屽瀛楃涓嶉敊浣嶏紱浣嶇疆 min() 闃茶秺鐣岋級銆佽緭鍏ユ潯 4 鍓嶇紑銆乵atcher 妯″紡璇嶅鐢ㄦ棦鏈夐敭锛?
  3 澶勫搴︾敱 len 鏀?UnicodeWidthStr::width
- `src/views/block_viewer/mod.rs`锛歴hortcuts_hints 12 澶?hint 鏍囩绾帴绾匡紙閿潎鍦?
  琛級銆?limit: "銆乺esult 璁℃暟涓ゆ潯锛堝崟澶嶆暟鎷嗛敭锛夈€?Sources ({})"
- `src/views/picker.rs`锛歋EARCH_BAR_LABEL 鍥涘鍑哄彛 tr锛堝竷灞€瀹藉害鏀规樉绀哄垪瀹斤級銆?
  " / to search"銆?Loading鈥?銆?No matches"锛沺icker_shortcuts 绛?hint 绾帴绾匡紱
  娉ㄦ剰 picker_shortcuts 鏄?LazyLock鈥斺€旇瘧鏂囬娆¤皟鐢ㄥ浐鍖栵紝璇█闅忓惎鍔ㄥ浐瀹氭晠鏃犲奖鍝?
- `src/slash/commands/theme.rs`锛歴uggest_args 鐨?"auto (follow system)" 涓?
  " (active)" 鍚庣紑 tr锛坉escription/usage 鐢?slash_meta! 瀹忕粺涓€鍖?tr 鏃犻渶閲嶅锛?
- `src/slash/commands/debug.rs`锛歴uggest_args 鍑哄彛 tr_str锛涗袱鏉℃柊閿紝绗笁鏉?
  scroll-diagnostics 宸插湪琛ㄧ函鎺ョ嚎
- `src/slash/mode_support.rs`锛歳efusal() 涓変釜妯℃澘鏁撮敭 + {why}/{instead} 鎻掑€煎
  tr锛? 鏉?why锛堝畾涔夊垎鏁ｅ湪 jump/dashboard/theme/find/timeline/tutorial 鍚勫懡浠?
  鏂囦欢锛変笌 1 鏉?instead 鍦ㄦ秷璐圭偣缁熶竴鏌ヨ〃锛屽悇瀹氫箟鏂囦欢闆舵敼鍔?

### 浠茶涓庡彇鑸嶏紙P4锛?

- `[cancel]`/`[Send now]`/`[edit]`锛坬ueue_pane 鎸夐挳缁勶級涓嶈瘧锛氬搴︽寜 ASCII
  `label.len()` 棰勭畻锛屼笁鎸夐挳 flush 閾句笌绐勯潰鏉夸涪鎸夐挳椤哄簭琚祴璇曟寜 ASCII 瀹芥柇瑷€
- `worktree ` 寰芥爣锛坙ocation.rs锛変笉璇戯細鐘舵€佹爮瀛敓瀹炵幇锛坅gent_view/render.rs锛?
  鎸?`"worktree ".width()` 棰勭畻璺緞鐑尯鍋忕Щ锛屼袱渚у繀椤诲悓杩涢€€
- `[Esc]`锛坆tw_overlay锛変笉璇戯細閿悕寰芥爣锛屽弬涓庡彸瀵归綈瀹藉害棰勭暀/鍛戒腑鐭╁舰
- 杩戦噸澶嶉敭骞跺瓨锛歚"New Worktree"`锛堟湰鏂囦欢鍘熸枃澶у啓锛変笌鏃㈡湁 `"New worktree"` 鍒嗛敭
  锛堝師鏂囦笉鍙敼锛夛紱`" search: "`锛坧icker 甯冨眬 pad锛変笌 `"search: "`锛坙ist_pane
  杈撳叆鏉★級鍒嗛敭锛沗"queued"` 灏忓啓涓?P3 `"Queued"` 鍒嗛敭
- 瀹藉害璇箟浼撮殢淇锛堣嫳鏂囬€愬€间笉鍙橈紝涓枃鎵嶇敓鏁堬級锛歭ist_pane toast/杈撳叆鏉?status銆?
  picker 鎼滅储鏉°€亀orkspace_mode trailing/Workspace 鍓嶈繘閲忓叡 6+ 澶?byte len 鈫?
  UnicodeWidthStr::width锛屽睘"璇戞枃淇濆"閾佸緥鐨勬垚瀵硅皟鏁?
- 妯″紡鍚?minimal/fullscreen锛坮efusal 妯℃澘 {current} 杩愯鏃跺€硷級鏈瘧锛氭ā寮?鍛戒护
  鏍囪瘑绗︼紝涓庢棦鏈夎〃姝ｆ枃鐢ㄦ瀬绠€/鍏ㄥ睆銆佸懡浠ゅ悕淇濈暀鍘熸枃鐨勮瘧娉曞苟瀛?
- screen_mode_switch.rs 闆舵敼鍔細璁″垝鎵€璁?婕忚瘧涓€鏉?鍓嶆彁涓嶆垚绔嬶紙涓ら敭鍧囧湪琛ㄤ笖宸插寘
  tr锛夛紱expand.rs 闆舵敼鍔細UseInstead 鎻愮ず鍦?mode_support 娑堣垂鐐圭粺涓€鏌ヨ〃
- credit_bar 缁撹鎬ц眮鍏嶈褰曪細鐘舵€佹潯娓叉煋璺緞褰撳墠鏈帴锛堟浠ｇ爜锛夛紝閿叆琛ㄥ鐢紝
  鏈潵鎺ョ嚎鍗崇敓鏁?

### 宸茬煡 Windows 鐜鏃忔祴璇曞け璐ワ紙涓?P4 鏃犲叧锛孉/B 瀹氳矗鐣欐。锛?

P4 鍚庤窇 `cargo test --lib views::` 涓?2766 閫氳繃 / 2 澶辫触锛涗袱澶辫触鍦?main 鍩虹嚎
锛?878bc8d锛孭4 涔嬪墠锛夊悓鏍峰け璐ワ紝涓?git -S 鑰冨彜纭缂洪櫡閫昏緫鍧囨潵鑷笂娓告彁浜?
锛坈68e39f6 棣栧彂 / a5589e95 鍚屾锛夛紝LOCAL 鍚勬湡鏈Е纰帮細

- `views::extensions_modal::tests::handle_key_tab_completes_single_field_path`锛?
  `tab_complete_path()`锛坋xtensions_modal.rs ~1620锛夌埗鐩綍鍥炴嫾鍙 `/`
  锛坄expanded.contains('/')` 鈫?`rsplit_once('/')`锛夛紝Windows 璺緞鏄?`\` 鍒嗛殧锛?
  parent_str 寰楃┖涓?鈫?Tab 琛ュ叏鍙墿鍩哄悕銆備笂娓?Linux CI 涓嶆挒鐨?Windows 鐪熺己闄?
  锛堝悓姝ユ枃浠讹紝淇椤荤櫥璁伴噸鏀撅級锛屽彟琛岀珛椤?
- `views::btw_overlay::tests::done_state_scans_file_paths_like_scrollback`锛?
  娴嬭瘯鐢?POSIX 璺緞 `/Users/...`锛屾壂鎻忎笌瑙ｆ瀽閾捐矾锛坥sc8.rs pass 2 鈫?
  resolve_tool_path_target锛夊叏閫氾紝鏈€鍚?`file_path_to_url` 鐨?
  `Url::from_file_path` 鍦?Windows 鎷掔粷鏃犵洏绗﹁矾寰?鈫?鏃?osc8_url銆侾OSIX 璇箟
  鍋囪鐨勫钩鍙板樊寮傦紝闈炰骇鍝佺己闄凤紙Windows 鐪熷疄璺緞甯︾洏绗︼紝璧?Prefix 鍒嗘敮姝ｅ父锛?

## 鍗佷簩鏈熷悗锛歩18n 鍒嗙骇鏂藉伐璁″垝锛圥0鈥揚4锛夊叏閮ㄥ畬宸?

鍚庣画鏂板 UI 鏂囨闅忓啓闅忚ˉ閿嵆鍙紱doctor 璇婃柇涓?tips 闈㈡澘銆乵arkdown 姝ｆ枃浠嶆槸
鑼冨洿澶栵紙瑙佹柟妗堟枃妗?鐮嶆帀/鍐崇瓥椤?锛夈€?

## 鍗佷笁鏈熻ˉ涓侊紙2026-09-18锛氫細璇濋粯璁?agent 灏婇噸 `[agent] name` 閰嶇疆锛?

### 闂

涓婃父榛樿 `plan_mode`/`ask_user`/`subagents` 鍏ㄥ紑锛坄app.plan_mode = !args.no_plan`锛?
event_loop.rs:1241-1243锛屾棤姝ｅ悜 `--plan` 鏃楁爣銆佹棤娉曡〃杈?鐢ㄦ埛鏄惧紡閫夋嫨"锛夛紝TUI 鍒涘缓
浼氳瘽鏃?`SessionFlags::agent_profile()`锛坅pp/effects/helpers.rs锛夋寜鏃楁爣鍚堟垚
`_meta.agentProfile = "grok-build-plan"`銆俿hell 瑙ｆ瀽閾撅紙xai-grok-shell
agent_ops.rs `resolve_agent_definition`锛変腑 ACP agentProfile锛堢 2 姝ワ級浼樺厛绾?
楂樹簬 config `[agent] name`锛堢 5 姝ワ級鈫?鐢ㄦ埛閰嶇疆鐨勯粯璁?agent 琚潤榛樿鐩栵紝
鏂颁細璇濇案杩滄槸 grok-build-plan銆?

### 鏀瑰姩锛坮espect_config_agent 闂搁棬锛?

- `src/app/effects/helpers.rs`锛歚SessionFlags` 澧?`respect_config_agent: bool`锛?
  鏂板鍏宠仈鍑芥暟 `config_default_agent_name()`锛堣鐢熸晥閰嶇疆鐨?`[agent] name`锛夛紱
  `to_meta()` 鐨?agentProfile 鍚堟垚鍒嗘敮鍔?`!respect_config_agent` 鍓嶇疆鏉′欢
- `src/app/event_loop.rs`锛歚session_flags_for_effects` 鏋勯€犵偣锛?
  `respect_config_agent = app.agent_override.is_none() && config_default_agent_name().is_some()`
- `src/app/effects/tests.rs`锛氭柊澧?
  `respect_config_agent_suppresses_synthesized_profile`锛堥椄闂ㄥ彧鎶戝埗 profile锛?
  涓嶅奖鍝?yoloMode 绛夊叾浣?meta锛?

### 琛屼负鐭╅樀

- 閰嶇疆鏃?`[agent] name`锛氳涓轰笌涓婃父涓€鑷达紙鍚堟垚 profile锛?
- 閰嶇疆鏈?`[agent] name` 涓旀棤 `--agent`/`GROK_AGENT`锛氫笉鍙?agentProfile 鈫?
  閰嶇疆 agent锛坓rok-build-concise锛夌敓鏁堬紱plan/ask-user 鍏朵綑 meta 涓嶅彈褰卞搷
- `--agent` / `GROK_AGENT`锛氭樉寮忛€夋嫨锛屼紭鍏堢骇鐓ф棫锛岄椄闂ㄨ嚜鍔ㄨ浣?
- 姣忔 create-session 璇讳竴娆￠厤缃紙鐢ㄦ埛鎵嬪姩瑙﹀彂锛岄潪鐑矾寰勶級

### 閲嶆斁娉ㄦ剰

- 涓夊鍧囦负鍚屾鏂囦欢锛涢椄闂ㄦ槸绾墠缃潯浠讹紝涓嶆敼鍙?agent_profile() 鏈韩鐨勬槧灏勮〃
- 宸茬煡杈圭晫锛氶厤缃?agent 鍚?plan 妯″紡涓嶅啀鑷姩甯?plan 宸ュ叿闆嗭紙enter/exit_plan_mode
  闅?plan 瀹氫箟璧帮級鈥斺€旈渶瑕?plan 宸ヤ綔娴佹椂鏄惧紡 `grok2 --agent grok-build-plan`

## 鍗佷簲鏈熻ˉ涓侊紙2026-09-18锛氭瀬绠€椋庢牸涓?agent 鍙樹綋瑙ｈ€︼紝`/style` 姝ｄ氦瑕嗙洊灞傦級

> 鑳屾櫙锛氬崄涓夋湡鐨勬瀬绠€妯″紡鎶婇鏍艰鍒欑粦姝诲湪 `grok-build-concise` 鍙樹綋閲岋紙鎹㈣浇浣?鎹㈠伐鍏?
> 闆?涓?AGENTS.md/MCP/瀛愪唬鐞嗭級锛岀敤鎴疯鐨勬槸"椋庢牸绾︽潫鍙犲姞鍒颁换浣?agent"銆?
> 鏈湡鎶婁袱鑰呮浜ゅ寲锛氶鏍?= 鍙寔涔呭寲寮€鍏崇殑瑕嗙洊灞傦紱concise 鍙樹綋鍥炲綊涓婃父鏈箟
> 锛堢槮宸ュ叿闆?+ COMPACT_SYSTEM_PROMPT + agents_md:false锛屼笉鍐嶅唴宓岃鍒欙級銆?

### 璇箟锛堢敤鎴峰畾涔夛級

- `/style`锛堟棤鍙傛暟锛? 鍦ㄥ紑/鍏抽棿鍒囨崲锛沗/style minimal` 寮哄埗寮€锛沗/style default`
  寮哄埗鍏?
- 鍒囨崲**鎸佷箙鍖?*锛坄<grok_home>/output_style.json`锛屽閿欒В鏋愶細缂烘枃浠?鎹熷潖=鍏筹級
- 姣忔 session spawn锛堜富浼氳瘽涓庡瓙浠ｇ悊鍚屼竴鏉¤矾寰勶級璇讳竴娆℃寔涔呭寲鐘舵€侊紝鎶婅鍒欏彔鍔犲埌
  绯荤粺鎻愮ず 鈫?涓嬩竴娆′細璇濆惎鍔ㄥ繀鐒剁敓鏁?
- **棣栨妯″瀷璋冪敤鍓?*鍒囨崲锛氬悓鏃舵敼鍐欏綋鍓嶄細璇濈殑 System 澶达紙`replace_system_head`锛?
  骞舵洿鏂?actor 涓婄殑 live 鏍囧織 鈫?鏈細璇濈珛鍗崇敓鏁?
- 棣栨妯″瀷璋冪敤鍚庡垏鎹細鍙寔涔呭寲锛?*褰撳墠浼氳瘽鎻愮ず璇嶄笉鍔?*锛堜笉鏀瑰啓宸插缓绔嬬殑浼氳瘽鍘嗗彶锛?
  鏃犵紦瀛樻姈鍔級锛沴ive 鏍囧織涓嶅啀缈昏浆
- 鍒ゅ畾闂搁棬 = actor 鐨?`first_model_call_done`锛坰ampler_turn 鐨?
  `record_response_token_usage` 涓?`log_terminal_failure` 涓や釜鏀跺彛缃綅鈥斺€旀垚鍔熶笌
  缁堟€佸け璐ラ兘绠?鍙戠敓杩囨ā鍨嬭姹?锛?
- agent/model 鍒囨崲閲嶅缓鎻愮ず璇嶆椂鎸?live 鏍囧織閲嶆斁瑕嗙洊灞傦紙骞傜瓑 strip+append锛夛紝椋庢牸
  璺ㄨ浇浣撳垏鎹繚鎸?

### 鏀瑰姩锛堟寜 crate锛?

- `xai-grok-agent/src/prompt/template.rs`锛歚LOCAL_CONCISE_RULES` 鏇村悕
  `LOCAL_MINIMAL_STYLE_RULES`锛涙柊 `apply_minimal_style(base, enabled)`锛堟寜
  `# Output style: minimal` 澶存埅鏂墺绂?+ 灏鹃儴杩藉姞锛屽弻鍚戝箓绛夛級+ 鍗曟祴
- `xai-grok-agent/src/config.rs`锛歚grok_build_concise()` 鐨?system_prompt 鍥炲綊
  绾?`COMPACT_SYSTEM_PROMPT`锛堣鍒欐敞鍏ョЩ闄わ級
- `xai-grok-shell/src/agent/output_style.rs`锛堟柊锛夛細鎸佷箙鍖栬/鍐?
  `<grok_home>/output_style.json`锛坄grok_home()` 鍚屾璺緞锛涘閿欒В鏋愶級+ 鍗曟祴
- `xai-grok-shell/src/session/acp_session.rs`锛歋essionActor 鏂板
  `output_style_applied: AtomicBool`锛坙ive 鏍囧織锛変笌 `first_model_call_done:
  AtomicBool`
- `xai-grok-shell/src/session/acp_session_impl/spawn.rs`锛歜ootstrap 澶?
  `apply_minimal_style(agent.system_prompt(), 鎸佷箙鍖栫姸鎬?`锛堜富/瀛愪唬鐞嗗叡浜矾寰勶紝
  鍏?agent 瑕嗙洊锛夛紱actor 鍒濆鍖栦袱涓瓧娈?
- `xai-grok-shell/src/session/acp_session_impl/sampler_turn.rs`锛氫袱涓敹鍙ｇ偣缃綅
  `first_model_call_done`
- `xai-grok-shell/src/session/acp_session_impl/model_switch.rs`锛?
  `handle_set_session_model` 鐨?concise 鐗逛緥鍒嗘敮鍒犻櫎锛坄use_concise` 鍙傛暟閫€鍖栦负
  `_use_concise`锛夛紝鏀瑰啓缁熶竴璧?`apply_minimal_style(agent.system_prompt(),
  output_style_applied)`锛沗handle_rebuild_agent_for_definition` 閲嶅缓鎻愮ず璇嶅悓鏍?
  鎸?live 鏍囧織閲嶆斁
- `xai-grok-shell/src/session/slash_commands.rs`锛歚BuiltinCommand "style"` 娉ㄥ唽
  锛坄BuiltinGate::AlwaysOn` 鑷姩杩涗繚鐣欏悕鍗曪級+ `BuiltinAction::SetMinimalStyle
  { enabled: Option<bool> }`锛圢one=鍒囨崲锛? `command_name`/`args_provided` 鑷?
- `xai-grok-shell/src/session/acp_session_impl/slash_exec.rs`锛氭墽琛屽櫒锛堟寔涔呭寲 +
  棣栨璋冪敤鍓?`replace_system_head` 閲嶅啓 + 缈昏浆 live 鏍囧織锛涘け璐ヤ粎鍛婅涓嶅洖婊氾級
- `xai-grok-pager/src/slash/i18n.rs`锛氬懡浠ゆ弿杩颁腑鏂囬敭 1 缁勶紙鎻忚堪缁?
  acp_command 鐨?tr_str 閫氶亾鑷姩缈昏瘧锛沘rgument_hint 涓?always-approve 鐨?
  "on|off" 鍚屼緥涓嶈瘧锛?

### 鍐崇瓥璁板綍 / 閲嶆斁娉ㄦ剰

- 瑕嗙洊灞傛槸**鏇挎崲寮?*锛坰trip 澶存埅鏂?+ append锛夛細鑻ヨ嚜瀹氫箟 agent 鎻愮ず璇嶆鏂囨伆濂藉惈
  `# Output style: minimal` 澶翠細琚埅鏂€斺€旇嚜瀹氫箟鎻愮ず璇嶉伩鍏嶄娇鐢ㄨ澶?
- 鎸佷箙鍖栨槸杩涚▼澶栨枃浠惰€岄潪 config.toml锛氶伩鍏嶇▼搴忓寲鏀瑰啓鐢ㄦ埛鎵嬬紪閰嶇疆锛涗笌
  announcements.json 鍚屾灏忕姸鎬佹枃浠舵ā寮?
- 鍘嬬缉涓嶉噸寤虹郴缁熷ご锛坄COMPACT_SYSTEM_PROMPT` 浠?concise 瀹氫箟涓?
  `compact_system_prompt()` 璁块棶鍣ㄧ敤锛屽悗鑰呮棤鐢熶骇璋冪敤鏂癸級鈥斺€斿帇缂╁悗椋庢牸淇濇寔锛屾棤闇€
  棰濆鎺ョ偣
- 鍗佷笁鏈熺殑"concise = compact 鍩哄骇 + 瑙勫垯鑺?璇箟鐢辨湰鏈熷彇浠ｏ細涓よ€呯幇鍦ㄥ彲鐙珛缁勫悎
  锛坈oncise 杞戒綋 + `/style` 寮€ = 绛変环鏃ц涓猴紱榛樿杞戒綋 + `/style` 寮€ = 鍏ㄥ伐鍏烽泦
  鏋佺畝椋庢牸锛孧CP/瀛愪唬鐞?AGENTS.md 涓嶅彈褰卞搷锛?
- 娴嬭瘯PROFILE 娉ㄦ剰锛歺ai-grok-shell 娴嬭瘯 profile 鏈変笂娓歌嚜甯︾紪璇戦敊璇紙瑙?宸茬煡
  闂"锛夛紝鏈湡 shell 渚ч獙璇佷互 `cargo check` 涓哄噯锛屽崟娴嬭鐩栨斁鍦?
  xai-grok-agent锛坅pply_minimal_style锛変笌 output_style.rs 妯″潡鍐?

### agent crate 娴嬭瘯鍩虹嚎褰掑洜锛圓/B 闈欐€佽瘉鏄庯紝涓庢湰鏈熸棤鍏筹級

`cargo test -p xai-grok-agent --lib` 10 澶辫触锛屽叏閮ㄤ笌鏈湡 diff 闆舵枃浠朵氦闆嗭紙diff
浠呰Е鍙?template.rs/config.rs + shell/pager 10 鏂囦欢锛屽け璐ユ祴璇曠殑杈撳叆鍦ㄥ垎鏀笌
main 閫愬瓧鑺傜浉鍚岋紝灞?main 鏃㈡湁 Windows 鐜鏃忥級锛?

- `prompt::template::test_encrypted_templates_not_stale`锛歚.gitattributes` 瀵?
  `templates/` 鏃?LF 瑙勫垯锛學indows CRLF checkout 浣?include_bytes! 璇诲埌 CRLF锛?
  涓?LF 鐢熸垚鐨勫姞瀵嗗父閲忎笉鍖归厤锛堜笂娓?`Synced from monorepo` 鑷甫锛?
- plugins::local_refresh / install_registry 7 渚嬶細symlink / 鏂囦欢閿?/ 涓存椂璺緞
  瀹舵棌
- prompt::skills 2 渚嬶細鐩綍鎵弿瀹舵棌

鏈湡瑙﹀強闈㈠叏閮ㄩ€氳繃锛歚minimal_style_overlay_appends_strips_and_is_idempotent`銆?
`test_mid_session_switch_concise_to_full`銆乼emplate 缁?31/32锛堝敮涓€澶辫触鍗充笂杩?
鏃㈡湁椤癸級銆?

## 鍗佸洓鏈熻ˉ涓侊紙2026-09-18锛氬浗妯￠€傞厤鈥斺€攖hink 鏍囪娉勬紡 + ChatCompletions 缃戝叧 quirk锛?

> 鑳屾櫙锛欴eepSeek V4 Flash 缁?OpenAI 鍏煎绔偣鎺ュ叆鏃讹紝`</think>` 缁撴潫鏍囪琚綋浣滄鏂?
> 娉勬紡鍒?UI锛堟帹鐞嗘湰浣撹蛋 `reasoning_content` 瀛楁锛岀綉鍏冲湪 reasoning鈫掓鏂囪竟鐣屾妸瀛ょ珛鐨?
> `</think>` 浣滀负鍗曠嫭 content chunk 涓嬪彂锛沷pencode issue #34126 鍚屾娴佸舰鎬侊級銆?
> 璋冪爺缁撹锛坥pencode #1325/#34698/#15389銆乿ercel/ai extractReasoningMiddleware銆?
> qwen-code taggedThinkingParser.ts銆乧rush/fantasy銆乿LLM ReasoningParser锛夛細鏈寸礌
> 瀛楃涓叉浛鎹㈠繀璐ヤ簬璺?chunk 鎷嗗垎锛圠iteLLM ollama 鍙嶉潰鏁欐潗锛夛紝姝ｇ‘鍋氭硶鏄法 chunk
> 娴佸紡鐘舵€佹満锛涘瓨鍌ㄥ眰淇濇寔"姝ｆ枃宸插墺绂?+ reasoning 鍗曠嫭鎴愰」"锛坥pencode 淇濈暀鍘熸枃鐨?
> 鍝插琚惁锛歡rok 鐨?content_acc 浼氬師鏍峰洖浼犱笅涓€杞紝鏍囪蹇呴』涓嶈繘瀛樺偍姝ｆ枃锛夈€?

### xai-grok-sampler
- 鏂?`src/stream/think_split.rs`锛歚ThinkTagSplitter` 涓ょ浉鐘舵€佹満锛坱ext/think锛夛紝
  鍠?content delta 浜у嚭 (text, reasoning) 浜屽厓缁勩€傝鐐癸細
  - 鍙屾爣璁伴泦 `<think>|<thinking>` / `</think>|</thinking>`锛坬wen-code 鍚屾锛夛紱
  - 璺?chunk holdback锛歜uffer 鏁翠綋鎴栨渶闀垮悗缂€鏄换涓€鏍囪鐨勭湡鍓嶇紑鏃舵墸鐣?
    锛坴ercel/ai getPotentialStartIndex 鍚屾锛涗笂闄?11 瀛楄妭锛夛紱
  - 杈圭晫瀛ゅ効鏍囪锛氬瓧娈?reasoning 宸茶 + 姝ｆ枃鏈紑濮嬶紙绾┖鐧戒笉绠楋級鏃讹紝瀛ょ珛鎴?
    鍓嶅 `</think>` 涓㈠純鈥斺€旀鏂囧紑濮嬪悗鍚屼覆鍘熸牱淇濈暀锛坥pencode #34698 鐨?
    绐勮涔?+ no-regression 涓ゆ潯娴嬭瘯鍚屾锛夛紱
  - EOF flush 涓嶄涪瀛楄妭锛氭湭闂悎 think 鍧楀綊 reasoning锛屾墸鐣欑殑鍗婃埅鏍囪褰掓鏂?
    锛坬wen-code `final` 璇箟锛屼慨 vercel/ai TransformStream 鏃?flush 鐨勫潙锛夛紱
  - 17 涓崟娴嬶紙鍚?#34126 娴佸舰鎬併€佽法 chunk 鎷嗗垎銆乣<think></think>` 绌哄潡銆?
    鍏堟鏂囧悗 think銆佸 think 鍧椼€乣value < than` 璇姤淇濇姢绛夛級銆?
- `src/stream/mod.rs`锛歚mod think_split;`锛圠OCAL 娉ㄩ噴澶勶級
- `src/stream/chat_completions.rs`锛?
  - reasoning 鍒嗘敮鎻愬埌 content 鍒嗘敮鍓嶏紙娣峰悎 delta 鏃跺厛姝﹁杈圭晫瑙勫垯锛夛紝
    `reasoning_content.or(reasoning).or(reasoning_text)` 褰掍竴鍖栵紙GLM/vLLM 鐨?
    `reasoning`銆並imi 鐨?`reasoning_text` 鍒悕瀛楁涓婂悓涓€閫氶亾锛夛紱
  - content 鍒嗘敮杩?`think_splitter.feed()`锛宺easoning 浜у嚭涓庡瓧娈典骇鍑哄悓璺細
    FirstToken/chunk_index/reasoning_acc/ChannelToken::Reasoning锛泃ext 浜у嚭鐓ф棫
    锛堟棤鏍囪娴侀€愬瓧鑺備笉鍙橈紝瀛橀噺娴嬭瘯闆舵敼鍔ㄩ€氳繃锛夛紱
  - 娴佸熬 `finish()` flush 灏惧反锛圥artial marker/鏈棴鍚堝潡锛夛紱
  - 宸ュ叿璋冪敤 id 缂哄け鏃跺悎鎴?`call_{index}`锛堥儴鍒嗙綉鍏充粠涓嶅彂 id锛岀粨鏋滄棤娉曢厤瀵癸級锛?
  - 娴嬭瘯杩藉姞 6 涓細#34126 杈圭晫娴佸舰鎬侊紙鏂█闆?Text token锛夈€佸唴鑱?think 璺?chunk銆?
    鍒悕瀛楁銆佹湭鐭?finish_reason serde銆丏eepSeek 缂撳瓨 token銆佺┖宸ュ叿 id 鍚堟垚銆?
- `src/stream_classify.rs`锛歚chat_chunk_has_content` 瑙ｆ瀯琛?`reasoning`/`reasoning_text`
  骞惰鍏?content 鍒ゅ畾锛圱TFT 闂ㄦ帶瀵瑰埆鍚嶅瓧娈典笉澶辨晥锛?

### xai-grok-sampling-types
- `src/types.rs`锛?
  - `ChatChunkDelta` 灏鹃儴杩藉姞 `reasoning: Option<String>` /
    `reasoning_text: Option<String>`锛坰erde default + skip_serializing_if锛?
  - `FinishReason` 杩藉姞 `#[serde(other)] Other` 鍙樹綋鈥斺€擠eepSeek 涓撴湁
    `insufficient_system_resources` 绛夋湭鐭?finish_reason 浼氳鏁翠釜 chunk 鍙嶅簭鍒楀寲
    澶辫触鏉€鎺夋暣鏉℃祦锛屾鍙樹綋鍏滀綇
  - `Usage` 灏鹃儴杩藉姞 `prompt_cache_hit_tokens` / `prompt_cache_miss_tokens`
    锛圖eepSeek 鎶婄紦瀛樺懡涓姤鎴愭墎骞?usage 瀛楁鑰岄潪
    `prompt_tokens_details.cached_tokens`锛?
- `src/conversation.rs`锛?
  - `From<FinishReason> for StopReason`锛歚Other => StopReason::Stop`锛堟渶璇氬疄鐨?
    杩戜技鏄犲皠锛?
  - `From<Usage> for TokenUsage`锛歚cached_prompt_tokens` 鍙?
    `details.cached_tokens.max(prompt_cache_hit_tokens)`锛堜袱濂楀瓧娈靛苟瀛樻椂鍙栧ぇ锛?

### 鍐崇瓥璁板綍
- Messages锛圓nthropic 鍏煎锛夊悗绔笉鏀癸細鎬濊€冭蛋缁撴瀯鍖?thinking block
  锛坄ThinkingDelta` 宸插鐞嗭級锛孌eepSeek Anthropic 绔偣涓嶄細鍐呰仈鏍囪杩?text
- Responses 鍚庣涓嶆敼锛氱粨鏋勫寲 reasoning item锛屽悓鐞嗗厤鐤紙opencode PR #34698
  鎻忚堪涓悓鏍风粨璁猴級
- 澶氳疆鍥炰紶涓嶉澶栧墺鍘嗗彶锛氭祦灞備慨澶嶅悗鏂?content 涓嶅啀鍚爣璁帮紱瀛橀噺浼氳瘽鍚爣璁扮殑
  鎸?opencode 浜夎鍐崇瓥淇濈暀鍘熸牱锛堟敼鍔ㄩ潰澶с€佹敹鐩婁笉纭畾锛?
- 涓嶅姞閰嶇疆寮€鍏筹細鏃犳爣璁版祦閫愬瓧鑺傜洿閫氾紙浠呭熬閮?`<` 绫诲墠缂€鐨勮法 chunk 鎵ｇ暀锛屼笅涓€
  chunk 鎴?EOF 蹇呯劧褰掕繕锛夛紝甯稿紑闆堕闄?
- 鐗堟湰锛歷1.0.35

## 瓒ｅ懗绛夊緟鏂囨锛坵itty loading phrases锛?026-09-20锛?

绉绘 qwen-code 绛夊緟妯″瀷鍝嶅簲鏃剁殑闅忔満瓒ｅ懗鏂囨锛圓pache-2.0锛涜瘝琛ㄤ笌杞崲鏈哄埗瑙?
`packages/cli/src/ui/hooks/usePhraseCycler.ts`銆乣packages/web-shell/client/constants/loadingPhrases.ts`銆?
`packages/cli/src/i18n/locales/zh.js` 鐨?`WITTY_LOADING_PHRASES`锛夈€傚垎鏀?
`feat/local-witty-loading-phrases`銆?

### xai-grok-pager
- `src/views/witty_phrases.rs`锛堟柊澧烇級锛欵N/ZH 鍏ㄩ噺璇嶈〃鍘熸枃杩佸叆 +
  `witty_phrase(turn_elapsed)`鈥斺€旀寜銆屾湰杞凡杩涜绉掓暟 / 15銆嶆椂闂存《 + 杩涚▼绉嶅瓙
  FNV-1a 鍙栬瘝锛堢‘瀹氭€т吉闅忔満锛屾棤 RNG 渚濊禆锛夛紝璇█鍙?`slash::i18n::current_lang()`
- `src/views/mod.rs`锛氭敞鍐?`witty_phrases` 妯″潡
- `src/views/turn_status.rs`锛歚compute_activity` 澧炲弬 `turn_elapsed`锛?
  `Thinking` / `Responding` 涓よ噦鐨勭姸鎬佽鏂囨浠庡浐瀹?"Thinking鈥? / "Responding鈥?
  鏀逛负瓒ｅ懗杞崲璇嶏紱鍏朵綑锛堝伐鍏疯繍琛?閲嶈瘯/Compacting/Waiting 瀹舵棌锛変笉鍔?
  锛坄Waiting(Model)` 鐨?"Waiting for response鈥? 鏈?pty e2e 渚濊禆锛屼繚鐣欙級
- 娴嬭瘯锛歚views::witty_phrases` 4 涓函鍗曟祴 + `turn_status` 鏃㈡湁鐢ㄤ緥鏀逛负鏂█璇嶈〃鎴愬憳
## /stats 妯″瀷鐢ㄩ噺鑱氬悎锛坢odel usage ledger锛?026-09-20锛?

`/stats` 鏂板鎸?5h/涓€鍛?涓€鏈?脳 model id 鑱氬悎鐨勭敤閲?鎬ц兘鎶ヨ〃锛堢疮璁¤緭鍏?杈撳嚭/缂撳瓨璇?
缂撳瓨鍐?token銆佸钩鍧囩紦瀛樺懡涓巼銆乀TFT 涓?TPS 鐨?p50/p90锛夛紝鍘嬬缉璐︽湰娈典繚鐣欏湪鍏跺悗銆?
鍒嗘敮 `feat/local-model-usage-stats`銆?

### xai-grok-tools
- `src/model_usage_ledger.rs`锛堟柊澧烇級锛氳皟鐢ㄧ矑搴?JSONL 璐︽湰
  `grok_home/cache/model-usage.jsonl`鈥斺€攕hell 姣忔鎴愬姛鎺ㄧ悊杩藉姞涓€鏉￠噰鏍?
  锛坱s/model_id/鍥涚被 token/ttft/tps/duration锛夛紝鏂囦欢瓒?2MB 鎸変繚鐣欑獥鍙ｏ紙35 澶╋級閲嶅啓鍓锛?
  `aggregate(samples, window_ms, now)` 绾嚱鏁拌仛鍚?+ 鏈€杩戦偦绉╃櫨鍒嗕綅

### xai-grok-shell
- `src/session/acp_session_impl/turn.rs`锛歚ModelResponseReceived` 浜嬩欢钀界洏鍚?
  杩藉姞涓€鏉?`model_usage_ledger` 閲囨牱锛坢odel_id 鏀逛负 clone 浼犲叆锛?

### xai-grok-pager
- `src/slash/commands/stats.rs`锛氶噸鍐欎负銆岀敤閲忚仛鍚堟姤琛?+ 鍘嬬缉璐︽湰銆嶅弻娈碉紱
  `format_usage_report(samples, now)` 绾嚱鏁板彲娴?
- `src/slash/i18n.rs`锛氬懡浠ゆ弿杩版崲閿?+ 13 涓姤琛ㄨ瘝鏉?

### 宸茬煡杈圭晫
- 閲囨牱鍙鐩栦富寰幆鎺ㄧ悊璺緞锛坰ubagent 鐨勬ā鍨嬭皟鐢ㄤ笉缁?turn.rs锛屾殏涓嶈鍏ワ級
- 鍘嗗彶鏁版嵁涓嶅洖濉細璐︽湰浠庡悎鍏ュ悗鏂颁骇鐢熺殑璋冪敤寮€濮嬬疮绉?
