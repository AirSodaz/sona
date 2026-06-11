# Sona ユーザーガイド

[English](user-guide.md) | [简体中文](user-guide.zh-CN.md) | [繁體中文](user-guide.zh-TW.md) | [日本語](user-guide.ja.md) | [プロジェクト README](../README.ja.md)

このガイドは、Sona をインストールし、初回セットアップを完了し、ローカル文字起こし、編集、任意の AI 機能、エクスポートまで進めたいデスクトップユーザー向けです。

## 1. Sona の用途

Sona は、音声からテキストへの作業を既定では自分のデバイス上に置きたい人のための、プライバシー重視の文字起こしエディタです。

Sona は次の用途に向いています。

- `Live Record` で会議、講義、インタビュー、メモを録音しながら文字起こしする
- `Batch Import` で既存の音声・動画ファイルを文字起こしする
- `Workspace`、`Projects`、`Inbox` で保存済みの録音やインポートを整理する
- タイムスタンプ、話者ラベル、セグメント単位のテキストを確認・編集する
- `Speaker Profiles` と `Speaker Review` で候補や匿名の話者グループを確認する
- 自分で設定したプロバイダーを使って `LLM Polish` や `Translate` を任意で実行する
- `Version Snapshots` から一括書き換え前の内容を復元する
- SRT、VTT、TXT、JSON などへエクスポートする
- `Voice Typing` で他のアプリへ直接入力する

`Live Caption` が主目的の場合は `Live Record` を確認してください。保存済みの作業を整理したい場合は `Workspace, Projects, And Inbox`、話者確認や復元が主目的の場合は `Transcript Editing And Playback`、`Voice Typing` が主目的の場合は `Export And Settings` の `Settings > Voice Typing` を確認してください。

## 2. インストールと起動

多くのユーザーにとっては、最新のリリースビルドを使うのが一番簡単です。

- [GitHub Releases](https://github.com/AirSodaz/sona/releases/latest) から Sona をダウンロードします。
- アプリを起動します。
- ソースからビルドする場合は、[プロジェクト README](../README.ja.md) の手順を使います。

初回起動時、Sona は `First Run Setup` で必要なローカル設定を案内します。セットアップを後回しにした場合、必要なオフライン設定が完了するまでリマインダーが表示されることがあります。

### CLI が必要な場合

コマンドラインからバッチ文字起こしを実行したい場合は、専用の [CLI ガイド](cli.ja.md) を参照してください。このユーザーガイドはデスクトップアプリの日常操作に絞っています。

## 3. First Run Setup

ローカル文字起こしを使うには、利用可能なオフラインモデル設定が必要です。

### 前提

- Sona が正常に起動できること。
- 推奨モデルパックをダウンロードする場合は、インターネット接続があること。

### 手順

1. Sona を起動し、`First Run Setup` が表示されるのを待ちます。
2. Welcome ステップを確認します。推奨される最初の成功パスは `Microphone -> Live Record` です。
3. `Continue` をクリックします。
4. モデルステップで `Download Recommended Models` をクリックします。必要なモデルがすでにある場合は `Continue` が表示されます。
5. 推奨オフラインモデルのダウンロードと展開が終わるまで待ちます。
6. マイクステップへ進み、マイクアクセスを許可します。
7. 権限を拒否した場合は、OS 側の権限を直してから `Try Permission Again` を使います。
8. `Live Record` に使うマイクを選びます。
9. `Start with Live Record` をクリックします。

### 結果

- Sona はローカル文字起こし向けの推奨オフライン設定を適用します。
- アプリは `Live Record` を開きます。
- セットアップが未完了の場合、リマインダーから onboarding を再開できます。

### メモ

- 推奨モデルパックは、ローカル文字起こしを早く動かすための最短ルートです。
- onboarding 中に `Later` を選んだ場合でも、リマインダーから戻れます。
- モデルはあとから `Settings > Model Settings` で変更できます。
- 既定のマイクは `Settings > Input Device` で変更できます。

## 4. Live Record

`Live Record` は、音声をリアルタイムに取り込みながら文字起こしセグメントを確認したいときに使います。

### 前提

- `First Run Setup` を完了している、または `Settings > Model Settings` で `Live Record Model` を設定していること。
- マイクから録音する場合は、OS 側でマイク権限が許可されていること。

### 手順

1. `Live Record` タブを開きます。
2. 録音前に入力ソースとして `Microphone` または `Desktop Audio` を選びます。
3. `Start Recording` をクリックします。
4. 波形とタイマーを見ながら録音状態を確認します。
5. セッションを終了せず一時停止する場合は `Pause`、録音を確定する場合は `Stop` を使います。
6. `Subtitle Mode` や `Language` を調整したい場合は `Parameter Settings` を開きます。
7. フローティング字幕が必要な場合は `Live Caption` をオンにします。
8. 字幕ウィンドウの最前面表示、クリック透過、サイズ、幅、色、起動時動作などは `Settings > Subtitle Settings` で調整します。

### `Live Caption` の役割

- `Live Caption` は `Live Record` ページ上の `System Audio Captions` トグルで、主にシステム音声をフローティング字幕で見たいときに使います。
- 録音を開始しなくても単独でオンにできます。あとから `Live Record` を開始した場合は、両方を並行して使えます。
- `Settings > Subtitle Settings` は字幕ウィンドウの見た目や動作を管理します。

### 結果

- 右側のエディタに文字起こしセグメントが表示されます。
- 録音中、Sona はそのセッションを `Workspace` の `Draft` として表示できます。
- 録音を停止すると、同じ Draft が完成した文字起こしとして保存されます。
- 事前にプロジェクトを開いていない場合、保存済みアイテムは通常 `Inbox` に入ります。

### メモ

- 既定では `Ctrl + Space` で録音開始 / 停止を切り替えます。
- 録音中は `Space` で一時停止 / 再開できます。
- `Parameter Settings` は文字起こし時の挙動を扱います。`LLM Polish` 全体の設定ではありません。

## 5. Batch Import

`Batch Import` は、既存の音声・動画ファイルを Sona にバックグラウンドで文字起こしさせたいときに使います。

### 前提

- `Settings > Model Settings` で `Batch Import Model` が設定されていること。
- ファイル形式が Sona の対応形式であること。

### 手順

1. `Batch Import` タブを開きます。
2. インポートエリアへファイルを drop するか、`Select File` をクリックします。
3. 1 つ以上のファイルをキューに追加します。
4. キューサイドバーとアクティブアイテムの状態を確認します。
5. 追加したいファイルがある場合は `Add More Files` を使います。
6. 新しい処理の `Subtitle Mode` や `Language` を変える場合は `Parameter Settings` を開きます。
7. ファイル処理が終わったら、メインエディタで文字起こしを確認します。

### 結果

- Sona は `Pending`、`Processing`、`Complete`、`Failed` の状態でキュー処理します。
- 完了したアイテムは、編集、Translate、エクスポートに進めるメインエディタへ読み込まれます。

### メモ

- オフラインの Batch Import 用モデルが未設定の場合、Sona はインポートを開始せず onboarding を案内します。
- `Settings > Model Settings` には `Batch VAD Segmentation`、`VAD Buffer Size`、`Max Concurrent Transcriptions` など Batch Import に関わる設定があります。
- Batch VAD を無効にすると、ローカル Batch Import はファイル全体をまとめて認識します。

## 6. Transcript Editing And Playback

文字起こしが作成された後は、エディタがテキスト確認、話者確認、再生同期、保存済みリビジョンの中心になります。

### 前提

- `Live Record`、`Batch Import`、または `Workspace` から文字起こしを開いていること。

### 手順

1. エディタのセグメント一覧を確認します。
2. タイムスタンプをクリックして、その時刻へ移動します。
3. セグメントテキストをダブルクリックするか編集アクションを使って編集を開始します。
4. `Enter` で現在のセグメントを保存します。
5. `Shift + Enter` でカーソル位置からセグメントを分割します。
6. merge アクションで次のセグメントと結合します。
7. delete アクションで確認後にセグメントを削除します。
8. `Ctrl + F` で文字起こし内を検索します。
9. 音声ファイルがある場合は audio player で再生、一時停止、シーク、速度、音量を操作します。
10. 話者 badge がある場合はクリックし、同じ話者グループへ `Speaker Profile` を割り当てたり、匿名ラベルへ戻したりできます。
11. エクスポート前に集中的に確認したい場合は、ヘッダーの `Speaker Review` を開きます。
12. 保存済みの非 Draft アイテムでは、ヘッダーの `Version Snapshots` から過去 snapshot と比較し、選択行または全文を復元できます。

### 結果

- 文字起こしはセグメント単位で編集できます。
- 再生位置と文字起こしナビゲーションは timestamp で同期します。
- 話者修正は同じ話者グループ全体に適用されます。
- Version Snapshot から復元する前に現在の内容も保存されるため、rollback もやり直しやすくなります。

### メモ

- エディタ toolbar は、セグメント編集中だけ表示されます。
- toolbar には `Undo`、`Redo`、`Bold`、`Italic`、`Underline`、セグメント分割があります。
- `Speaker Profiles` は `Settings > Vocabulary` で作成します。project settings では、そのプロジェクトで有効にする profile を選べます。
- `Version Snapshots` は、`LLM Polish`、`Translate`、`Re-transcribe`、snapshot 復元前など、一括変更の前に作られます。

## 7. LLM Polish, Translation, And Summary

Sona の LLM 機能は任意です。ローカル文字起こしは LLM なしでも動作しますが、`LLM Polish`、`Translate`、`AI Summary` には `Settings > LLM Service` の設定が必要です。

### 前提

- 文字起こしセグメントがあること。
- 使いたい機能が `Settings > LLM Service` で設定されていること。

### LLM Service Setup

1. `Settings > LLM Service` を開きます。
2. `Feature Models` で `Polish Model`、`Translation Model`、`Summary Model` を選びます。
3. `Provider Credentials` で使うプロバイダーを開き、`Base URL`、`API Key`、`Endpoint`、`Deployment Name` など必要な接続情報を入力します。
4. 選択したモデルが対応している場合は、`Reasoning Mode` と `Reasoning Level` を設定します。
5. 認証情報を入力したら `Test Connection` を実行します。
6. 必要な feature model を割り当てたら、メイン画面へ戻ります。

### `LLM Polish` の手順

1. `Settings > LLM Service` で `Polish Model` が割り当てられていることを確認します。
2. `LLM Polish` ボタンをクリックします。
3. 必要なアクションを選びます: `LLM Polish`、`Re-transcribe`、`Undo`、`Redo`、`Advanced Settings`。
4. `Auto-Polish`、`Auto-Polish Frequency`、`Keywords`、`Scenario Presets`、`Custom Context` を調整したい場合は `Advanced Settings` を開きます。

### `Translate` の手順

1. `Settings > LLM Service` で `Translation Model` が割り当てられていることを確認します。
2. `Translate` ボタンをクリックします。
3. 翻訳先言語を選びます。
4. `Start Translation` または `Retranslate` をクリックします。
5. `Show Translations` / `Hide Translations` でエディタ内の二言語表示を切り替えます。

### `AI Summary` の手順

1. `AI Summary` が有効で、`Summary Model` が割り当てられていることを確認します。
2. セグメントがある文字起こしを開きます。
3. 文字起こし画面から summary editor を開きます。
4. 次回生成に使う summary template を変えたい場合は `General`、`Meeting`、`Lecture` を選びます。
5. `Generate` で要約を作成し、あとで置き換える場合は `Regenerate` を使います。
6. 必要に応じて要約本文を直接編集します。フォーカスが外れると auto-save されます。
7. 他の場所で使う場合は `Copy` をクリックします。

### 結果

- `LLM Polish` は文字起こし本文を直接更新します。
- `Translate` はセグメントごとに翻訳テキストを保存し、原文の下に表示できます。
- `AI Summary` は文字起こし本文を書き換えず、横に 1 件の現在の要約を保持します。
- 保存済みの非 Draft 文字起こしでは、一括変更前に `Version Snapshots` が作られます。

### メモ

- `Polish Model`、`Translation Model`、`Summary Model` は別々に設定します。同じプロバイダーでまとめても、機能ごとに分けてもかまいません。
- Translate は `Google Translate (Free)` や `Google Translate (API)` などの専用翻訳プロバイダーも使えますが、`LLM Polish` には LLM 対応プロバイダーとモデルが必要です。
- `AI Summary` にも LLM 対応プロバイダーとモデルが必要です。Google Translate 系プロバイダーは summary には対応していません。
- `AI Summary` が完全に設定されていなくても、summary editor で手動入力や編集はできます。
- 翻訳先言語には `Chinese (Simplified)`、`English`、`Japanese`、`Korean`、`French`、`German`、`Spanish` があります。
- 変更が大きすぎた場合は、`Version Snapshots` から選択行または全文を復元できます。

## 8. Export Transcript

文字起こしに 1 つ以上のセグメントがある場合、エクスポートできます。

### 手順

1. ヘッダーの `Export` をクリックします。
2. `Export Transcript` modal で `Filename` を入力します。
3. `Export Directory` を選びます。
4. 出力形式を選びます: `SubRip (.srt)`、`WebVTT (.vtt)`、`JSON (.json)`、`Plain Text (.txt)`。
5. export mode を選びます: `Original`、`Translation`、`Bilingual`。
6. `Export` をクリックします。

### 結果

- Sona は選択したパスと形式で文字起こしを書き出します。
- 翻訳がある場合は、翻訳のみまたは二言語形式で出力できます。

### メモ

- `Translation` と `Bilingual` は、翻訳テキストを持つセグメントがある場合だけ使えます。
- `Original` は常に利用できます。
- 話者ラベルが重要な場合は、エクスポート前に `Speaker Review` を実行してください。

## 9. Workspace, Projects, And Inbox

保存済みの録音やインポートを整理し、メイン編集画面から大きく離れず作業を続けたい場合は `Workspace` を使います。

### 各 scope の意味

- `All Items` は `Inbox` とすべての project を横断して表示します。
- `Inbox` は、まだ project に割り当てていない録音やインポートの既定の置き場です。
- project は、作業ごとのまとまり、project-specific defaults、新規作業への entry point を提供します。

### 手順

1. `Workspace` を開きます。
2. 左 rail で `All Items`、`Inbox`、各 project を切り替えます。
3. recurring work 用の場所が必要な場合は `New Project` をクリックします。
4. project 名と任意の説明を入力して作成します。新規 project は custom icon なしで始まります。
5. 新しい録音やインポートを project に紐づけたい場合は、その project を開いてから開始します。
6. project header のボタンから `Live Record` や `Batch Import` を開始できます。
7. 保存済み item をクリックすると、`Workspace` 内の detail pane で開いて編集を続けられます。
8. search、filters、sort、`List View` / `Grid View` / `Table View` で表示を絞ります。
9. selection mode を使うと、item を `Inbox` と project の間で移動したり、複数削除したりできます。

### Project Settings

1. project を開いて `Project Settings` をクリックします。
2. project 名と説明を更新します。
3. 必要に応じて project icon を選びます。
4. project 内で使う既定値を選びます: `Default Summary Template`、`Default Translation Language`、`Default Polish Scenario`、任意の `Default Polish Context`、`Export Filename Prefix`。
5. `Text Replacement`、`Hotword`、`Polish Keyword`、`Speaker Profile` sets を project ごとに有効 / 無効にできます。
6. `Save` で保存します。project だけを削除して中の item を `Inbox` に戻す場合は `Delete Project` を使います。

### メモ

- project を開かずに録音やインポートを始めると、新しい item は既定で `Inbox` に保存されます。
- active live recording は、停止前でも `Draft` として表示されることがあります。
- project icon は system icons、recommended emoji、custom emoji に対応します。
- project から離れるときに settings drawer に未保存変更がある場合、Sona は破棄するか確認します。

## 10. Export And Settings

ファイルを書き出す準備ができたら `Export` を使います。アプリ全体の既定値や補助機能は `Settings` で管理します。保存済みの作業を開き直したり整理したりしたい場合は、別の保存画面ではなく `Workspace` に戻ります。

### Settings で確認する主な場所

- `Settings > Dashboard`: 全体概要、話者 coverage、LLM 利用傾向。
- `Settings > General`: theme、language、font、tray behavior、update checks、`Diagnostics`、`Backup & Restore`。
- `Settings > Input Device`: microphone、system audio、microphone boost、recording 中 mute。
- `Settings > Subtitle Settings`: Live Caption の起動、クリック透過、最前面表示、font size、width、color、背景透明度。
- `Settings > Voice Typing`: `Voice Typing` の有効化、global shortcut、`Push to Talk (Hold)` / `Toggle (Press once)`、readiness。
- `Settings > Model Settings`: `Live Record Model`、`Batch Import Model`、`Transcription Settings`、`ITN`、`Batch VAD Segmentation`、`VAD Buffer Size`、`Max Concurrent Transcriptions`、downloadable models。
- `Settings > Vocabulary`: `Text Replacement`、`Hotwords`、polish keyword sets、polish context presets、summary templates、`Speaker Profiles`。
- `Settings > Automation`: Sona 起動中に新しい media を監視し、transcribe、polish、translate、export を実行する watched-folder rules。
- `Settings > LLM Service`: feature model bindings、reasoning options、provider credentials。
- `Settings > Shortcuts`: Live Record、playback、search、workspace navigation、editor shortcuts。
- `Settings > About`: source code、logs、update 関連 actions。

### `Voice Typing`

- `Voice Typing` は、chat apps、documents、forms など他のアプリへ直接 dictation したいときに使います。
- `Settings > Voice Typing` を開き、`Voice Typing` をオンにして global shortcut を設定し、`Push to Talk (Hold)` または `Toggle (Press once)` を選びます。
- 短い入力には `Push to Talk (Hold)`、長めの dictation には `Toggle (Press once)` が向いています。
- `Voice Typing` は同じオフライン live transcription setup を使うため、`Live Record Model`、必要な `VAD` model、利用可能な input device が必要です。

### Diagnostics And Backup

- `Settings > General` の `Diagnostics` では、ローカル文字起こし chain、runtime readiness、packaging environment を確認できます。
- 同じ画面の `Backup & Restore` では、config、workspace、軽量な history transcripts と summaries、automation state、dashboard LLM usage を含む light archive を export / import できます。
- light backup archive は text history と summaries を復元しますが、original audio files は含みません。復元した item は audio playback なしで開く場合があります。
- `WebDAV Cloud Sync` は `Backup & Restore` 内にあり、この device に credentials を保存して backup archive を手動 upload / restore できます。

### Notification Center

- ヘッダー notification center は、updates、`Recovery Center`、automation results をまとめます。
- 中断した batch / automation work、relaunch が必要な update、automation success / failure を確認するときに使います。
- `Recovery Center` では、中断した batch / automation item を resume または discard できます。`Diagnostics` と `Backup & Restore` は `Settings > General` にあります。

## 11. FAQ And Troubleshooting

### Sona が setup completion を何度も求めます

- onboarding banner を開き、model と microphone の step を完了してください。
- 以前 skip した場合は、`Settings > Model Settings` で `Live Record Model` と `Batch Import Model` の両方が設定されているか確認してください。

### `Live Record` を開始できません

- OS の microphone permission を確認します。
- `Live Record Model` が設定されているか確認します。
- 入力ソースが `Microphone` または `Desktop Audio` として正しいか確認します。

### `Batch Import` を開始できません

- `Batch Import Model` が設定されているか確認します。
- ファイル拡張子が対応形式か確認します。
- unsupported format が表示された場合は、ファイルを変換してから再試行してください。

### `LLM Polish` または `Translate` が disabled になる、または失敗します

- `Settings > LLM Service` に正しい provider credentials があるか確認します。
- `Polish Model` または `Translation Model` が feature に割り当てられているか確認します。
- retry 前に `Test Connection` を実行します。
- `Ollama` など custom endpoint / local service を使う場合は、その service 自体が動作しているか確認してください。

### `Auto-Polish` が見つかりません

- `LLM Polish > Advanced Settings` で `Auto-Polish`、frequency、keywords、scenario presets、custom context を管理します。

### export に `Original` しか表示されません

- `Translation` と `Bilingual` は、文字起こしに translation text がある場合だけ表示されます。

### `Speaker Review` が空、または候補がありません

- `Speaker Review` は既存の speaker metadata を group 化します。話者情報なしで作成された文字起こしでは、確認対象がない場合があります。
- 候補は `Settings > Vocabulary` の `Speaker Profiles`、imported reference samples、current project で有効な profiles に依存します。
- speaker label が見えている場合は、エディタ上の badge から手動で profile を割り当てられます。

### `Version Snapshots` が見つかりません

- `Version Snapshots` は、transcript segments を含む保存済み workspace item に対して表示されます。
- temporary `current` transcript や進行中の live recording draft では hidden です。
- snapshots は `LLM Polish`、`Translate`、`Re-transcribe`、snapshot restore 前など一括変更の前に作られます。

### 復元した backup は text だけ開き、audio playback がありません

- backup archive は軽量化されています。config、workspace data、light history transcripts と summaries、automation state、dashboard LLM usage は含みますが、original audio files は含みません。
- そのため、復元した entry は読んだり編集したりできますが、source audio が別経路で利用可能になるまで playback できない場合があります。

### 新しい item が最初に `Inbox` に表示されるのはなぜですか

- `Inbox` は、まだ project に割り当てていない録音やインポートの既定の置き場です。
- 新しい item を特定 project に保存したい場合は、`Live Record` や `Batch Import` を始める前にその project を開いてください。
- あとから `Workspace` で `Inbox` から project へ移動することもできます。

### project を削除すると何が起きますか

- project を削除しても、その中にあった recordings や imports は削除されません。
- Sona はそれらを `Inbox` に戻すため、あとから再割り当てできます。

### 録音中に `Draft` item が見えるのはなぜですか

- active live recording 中、Sona はその session の保存先として `Draft` item を作ることがあります。
- 録音を停止すると、同じ item が完成済み entry になります。

### `Live Caption` が表示されません

- `Live Record` ページに戻り、`Live Caption` がオンになっているか確認してください。`Settings > Subtitle Settings` は window behavior と appearance だけを管理します。
- system-audio の floating subtitle だけが目的なら、録音を始める必要はありません。`Live Caption` をオンにすれば十分です。
- `Live Caption` は同じ offline live transcription setup に依存するため、`Live Record Model` が設定されている必要があります。

### Voice Typing が動きません

- `Settings > Voice Typing` で `Voice Typing` をオンにします。
- shortcut、model、VAD、input device、readiness state が期待どおりか確認します。
- Voice Typing は同じ offline transcription setup を使うため、live transcription model が必要です。

### 中断した batch / automation work はどこから復元しますか

- Sona が pending recovery items を通知した場合は、ヘッダー notification center を開きます。
- `Recovery Center` で中断した batch / automation work を resume または discard します。
- Diagnostics や backup を探している場合は `Settings > General` を開いてください。

### playback controls が見当たりません

- audio player は、現在の文字起こしに保存済み録音や処理済みファイルなどの audio source がある場合だけ表示されます。

### Sona を build / develop したい

- source build と development commands は [プロジェクト README](../README.ja.md) を参照してください。
