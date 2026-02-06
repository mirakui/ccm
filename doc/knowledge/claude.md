# Claude Code と TTY の連携

---

## 目次

1. [TTY 検出とモード切り替え](#1-tty-検出とモード切り替え)
2. [インタラクティブモード](#2-インタラクティブモード)
3. [非インタラクティブモード（-p / --print）](#3-非インタラクティブモード-p----print)
4. [stdin/stdout/stderr の扱い](#4-stdinstdoutstderr-の扱い)
5. [シェルスクリプトとパイプライン連携](#5-シェルスクリプトとパイプライン連携)
6. [ターミナルマルチプレクサ連携](#6-ターミナルマルチプレクサ連携)
7. [環境変数](#7-環境変数)
8. [Agent SDK（プログラマティックアクセス）](#8-agent-sdkプログラマティックアクセス)
9. [既知の制限と回避策](#9-既知の制限と回避策)
10. [Plan モード](#10-plan-モード)

---

## 1. TTY 検出とモード切り替え

Claude Code は標準の `isatty()` システムコールで TTY を検出し、モードを自動的に切り替える。

| 条件 | モード | 挙動 |
|------|--------|------|
| TTY あり | インタラクティブ | React + Ink ベースの TUI を起動。REPL で対話的に操作 |
| TTY なし or `-p` | 非インタラクティブ | 単発実行して終了。パイプ・スクリプト向け |

- TUI は **React + Ink**（ターミナル向け React レンダラ）で構築されている
- Ink は raw mode やターミナル制御のために TTY を必要とする
- TTY がない環境では TUI 機能（スピナー、カラー、Markdown レンダリング）が無効になる
- **既知の問題**: `-p` フラグ使用時でも TTY を要求するバグがある（[#9026](https://github.com/anthropics/claude-code/issues/9026)）

---

## 2. インタラクティブモード

`claude` または `claude "prompt"` で起動するデフォルトモード。

### REPL と対話

- マルチターン会話のセッション永続化
- スラッシュコマンド: `/commit`, `/vim`, `/terminal-setup`, `/config`, `/theme`, `/statusline` など
- Bash モード: `!` プレフィックスでシェルコマンドを直接実行（出力は会話コンテキストに追加）

### キーボードショートカット

| キー | 機能 |
|------|------|
| `Ctrl+C` | 入力/生成のキャンセル |
| `Ctrl+D` | セッション終了（EOF） |
| `Ctrl+G` | デフォルトテキストエディタで開く |
| `Ctrl+L` | ターミナル画面クリア（会話履歴は保持） |
| `Ctrl+O` | verbose 出力の切り替え |
| `Ctrl+R` | コマンド履歴の逆検索 |
| `Ctrl+V` / `Cmd+V`（iTerm2）/ `Alt+V`（Windows） | クリップボードから画像ペースト |
| `Ctrl+B` | バックグラウンド実行 |

### マルチライン入力

| 方法 | 対応環境 |
|------|----------|
| `\` + Enter | すべてのターミナル |
| `Shift+Enter` | iTerm2, WezTerm, Ghostty, Kitty（ネイティブ対応） |
| `Option+Enter` | macOS デフォルト |
| `Ctrl+J` | ラインフィード |

VS Code, Alacritty, Zed, Warp では `/terminal-setup` コマンドで `Shift+Enter` を設定する必要がある。

### Vim モード

`/vim` または `/config` で有効化。`Esc` で NORMAL モード、`i`/`I`/`a`/`A` で INSERT モードに切り替え。

### Option/Alt キーの設定

`Alt+B`, `Alt+F` などのショートカットを使うには、Option キーを Meta として設定する必要がある：

- **iTerm2**: Settings → Profiles → Keys → Left/Right Option key を "Esc+" に設定
- **Terminal.app**: Settings → Profiles → Keyboard → "Use Option as Meta Key" にチェック

---

## 3. 非インタラクティブモード（-p / --print）

単発実行してスクリプトやパイプラインで利用するモード。

### コアフラグ

| フラグ | 説明 |
|--------|------|
| `-p` / `--print` | 単発実行して終了 |
| `-c` / `--continue` | 現在ディレクトリの最新会話を継続 |
| `-r` / `--resume` | ID または名前でセッションを再開 |
| `--output-format` | `text`（デフォルト）, `json`, `stream-json` |
| `--input-format` | `text`（デフォルト）, `stream-json` |
| `--verbose` | ターンごとの詳細ログ |
| `--include-partial-messages` | ストリーミング出力に部分メッセージを含める（`stream-json` 時） |

### 権限とツール制御

| フラグ | 説明 |
|--------|------|
| `--allowedTools` | ツールの事前承認（例: `"Write" "Bash(git *)"` ） |
| `--disallowedTools` | ツールのブロック（例: `"Bash(rm *)"` ） |
| `--dangerously-skip-permissions` | すべての権限プロンプトをスキップ |
| `--permission-prompt-tool` | 非インタラクティブで権限判定する MCP ツール |

### システムプロンプト

| フラグ | 説明 |
|--------|------|
| `--system-prompt` | システムプロンプト全体を置き換え |
| `--system-prompt-file` | ファイルからシステムプロンプトを読み込み |
| `--append-system-prompt` | デフォルトプロンプトに追記 |
| `--append-system-prompt-file` | ファイル内容をデフォルトプロンプトに追記 |

### セッションとコンテキスト

| フラグ | 説明 |
|--------|------|
| `--session-id` | セッション ID を指定（有効な UUID） |
| `--no-session-persistence` | セッション永続化を無効化 |
| `--fork-session` | 元のセッションをフォークして新規 ID で実行 |
| `--add-dir` | 追加のワーキングディレクトリ |

### モデルとコスト制御

| フラグ | 説明 |
|--------|------|
| `--model` | モデル指定（エイリアス: `sonnet`, `opus` ） |
| `--fallback-model` | オーバーロード時の自動フォールバック |
| `--max-budget-usd` | 最大支出（ドル） |
| `--max-turns` | エージェントターン数の上限 |

### 構造化出力

| フラグ | 説明 |
|--------|------|
| `--json-schema` | JSON Schema によるバリデーション付き構造化出力 |

### 終了コード

| コード | 意味 |
|--------|------|
| `0` | 成功 |
| `2` | ブロッキングエラー（hooks コンテキストでは stderr が Claude にフィードバック） |
| その他の非ゼロ | 非ブロッキングエラー |

---

## 4. stdin/stdout/stderr の扱い

### stdin（パイプ入力）

```bash
cat logs.txt | claude -p "explain these errors"
gh pr diff "$1" | claude -p "review this"
echo "prompt" | claude -p "process this"
```

- パイプ経由でデータを渡すとコマンドライン長制限を回避できる
- `--input-format stream-json` でストリーミング JSON 入力にも対応
- インタラクティブモードでのプログラマティック入力は制限あり（[#15553](https://github.com/anthropics/claude-code/issues/15553)）

### stdout（出力）

```bash
# プレーンテキスト（デフォルト）
claude -p "summarize project"

# JSON 出力
claude -p "summarize project" --output-format json | jq '.result'

# ストリーミング JSON
claude -p "write a poem" --output-format stream-json
```

### stderr

- hooks で exit code 2 を返すと stderr が Claude にフィードバックされる
- 非ゼロ exit code の hooks は verbose モードで stderr を表示
- debug/verbose モードの出力が stderr に出ない既知バグあり（[#4859](https://github.com/anthropics/claude-code/issues/4859)）

---

## 5. シェルスクリプトとパイプライン連携

### 基本パターン

```bash
# ファイル分析
cat file.txt | claude -p "analyze this"

# PR レビュー
gh pr diff "$1" | claude -p "review for security"

# テストエラーの修正
npm test 2>&1 | claude -p "fix these errors"
```

### 出力の加工

```bash
# JSON 出力をファイルに保存
claude -p "summarize project" --output-format json > output.json

# jq で結果を抽出
claude -p "find bugs" --output-format json | jq '.result'
```

### リアルタイムストリーミング

```bash
claude -p "write a poem" --output-format stream-json \
  --verbose --include-partial-messages | \
  jq -rj 'select(.type == "stream_event" and .event.delta.type? == "text_delta") | .event.delta.text'
```

### 構造化データ抽出

```bash
claude -p "Extract function names from auth.py" \
  --output-format json \
  --json-schema '{"type":"object","properties":{"functions":{"type":"array","items":{"type":"string"}}}}' \
  | jq '.structured_output'
```

### セッション継続パイプライン

```bash
session_id=$(claude -p "Start a review" --output-format json | jq -r '.session_id')
claude -p "Continue that review" --resume "$session_id"
```

### カスタムシステムプロンプト

```bash
gh pr diff "$1" | claude -p \
  --append-system-prompt "You are a security engineer. Review for vulnerabilities." \
  --output-format json | jq -r '.result'
```

### 注意事項

- スラッシュコマンド（`/commit` など）はインタラクティブモードでのみ利用可能
- 非常に大きなパイプ入力はトランケートされる可能性がある（特に VS Code ターミナル）。ファイルに書き出して Claude に読ませるほうが確実

---

## 6. ターミナルマルチプレクサ連携

### tmux

- **セッション永続化**: ターミナルを閉じてもセッションが維持され、再アタッチ可能
- ペイン分割で Claude Code・ログ・テストを同時に表示
- `Ctrl+B` でバックグラウンド実行（tmux プレフィックスキーと競合するため 2 回押しが必要）
- **[tmux-mcp](https://github.com/nickgnd/tmux-mcp)**: MCP 経由で Claude が tmux セッションを読み書き・制御できるサーバー

### WezTerm

- `Shift+Enter` がネイティブ対応（設定不要）
- `/terminal-setup` コマンドは WezTerm では不要なため表示されない
- **CLI コマンドによる連携**:
  - `wezterm cli get-text` -- ターミナル内容の読み取り
  - `wezterm cli send-text` -- コマンド送信
  - `wezterm cli activate-pane` -- ペインのアクティブ化
- `$WEZTERM_PANE` 環境変数でペインを識別
- BSP（Binary Space Partitioning）レイアウトによるマルチセッション

### Shift+Enter 対応状況

| ネイティブ対応 | `/terminal-setup` が必要 |
|---------------|--------------------------|
| iTerm2, WezTerm, Ghostty, Kitty | VS Code, Alacritty, Zed, Warp |

---

## 7. 環境変数

### ターミナル検出と設定

| 変数 | 用途 |
|------|------|
| `COLORTERM=truecolor` | 24-bit RGB カラーを有効化 |
| `NO_COLOR=1` | カラーを完全無効化 |
| `CLAUDE_CODE_REMOTE=true` | リモート Web 環境での設定 |
| `WEZTERM_PANE` | WezTerm ペイン識別子 |

### バックグラウンドタスクと機能

| 変数 | 用途 |
|------|------|
| `CLAUDE_CODE_DISABLE_BACKGROUND_TASKS=1` | バックグラウンドタスクを無効化 |
| `CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false` | プロンプトサジェストを無効化 |
| `CLAUDE_CODE_TASK_LIST_ID=my-project` | セッション横断の名前付きタスクリスト |

### MCP とツール制御

| 変数 | 用途 |
|------|------|
| `MAX_MCP_OUTPUT_TOKENS=50000` | MCP ツール出力上限（デフォルト 25,000） |
| `MCP_TIMEOUT=10000` | MCP サーバー起動タイムアウト（ミリ秒） |
| `ENABLE_TOOL_SEARCH=auto\|true\|false` | MCP Tool Search の挙動制御 |

### API プロバイダ選択

| 変数 | 用途 |
|------|------|
| `ANTHROPIC_API_KEY` | Anthropic の API キー |
| `CLAUDE_CODE_USE_BEDROCK=1` | Amazon Bedrock を使用 |
| `CLAUDE_CODE_USE_VERTEX=1` | Google Vertex AI を使用 |
| `CLAUDE_CODE_USE_FOUNDRY=1` | Microsoft Azure AI Foundry を使用 |

### 注意

- 環境変数は全体で約 **70 個**（常用は約 20 個）
- **起動時に一度だけ読み込まれキャッシュされる**。変更後は再起動が必要

---

## 8. Agent SDK（プログラマティックアクセス）

Claude Code SDK は **Claude Agent SDK** にリネームされた。CLI・Python・TypeScript パッケージとして提供される。

### インストール

```bash
npm install @anthropic-ai/claude-agent-sdk  # TypeScript
pip install claude-agent-sdk                 # Python
```

### Python

```python
import asyncio
from claude_agent_sdk import query, ClaudeAgentOptions

async def main():
    async for message in query(
        prompt="Find and fix the bug in auth.py",
        options=ClaudeAgentOptions(allowed_tools=["Read", "Edit", "Bash"])
    ):
        print(message)

asyncio.run(main())
```

### TypeScript

```typescript
import { query } from "@anthropic-ai/claude-agent-sdk";

for await (const message of query({
  prompt: "Find and fix the bug in auth.py",
  options: { allowedTools: ["Read", "Edit", "Bash"] }
})) {
  console.log(message);
}
```

### 主な特徴

- **TTY 不要**: プログラマティックに完全制御可能
- **ビルトインツール**: Read, Write, Edit, Bash, Glob, Grep, WebSearch, WebFetch, AskUserQuestion
- **Hooks**: `PreToolUse`, `PostToolUse`, `Stop`, `SessionStart`, `SessionEnd`, `UserPromptSubmit`
- **サブエージェント**: 専門タスクの委譲
- **MCP サーバー統合**
- **セッション管理**: resume / fork
- **構造化出力**: JSON Schema バリデーション

### リポジトリ

- Python: [claude-agent-sdk-python](https://github.com/anthropics/claude-agent-sdk-python)
- TypeScript: [claude-agent-sdk-typescript](https://github.com/anthropics/claude-agent-sdk-typescript)
- デモ: [claude-agent-sdk-demos](https://github.com/anthropics/claude-agent-sdk-demos)
- ドキュメント: [Agent SDK Overview](https://platform.claude.com/docs/en/agent-sdk/overview)

---

## 9. 既知の制限と回避策

### 主要な既知バグ

| Issue | 概要 |
|-------|------|
| [#9026](https://github.com/anthropics/claude-code/issues/9026) | `-p` フラグ使用時でも TTY を要求してハングする |
| [#13598](https://github.com/anthropics/claude-code/issues/13598) | `/dev/tty` の spurious reader がブロッキング読み取りを開始し、入力を奪う |
| [#15553](https://github.com/anthropics/claude-code/issues/15553) | プログラマティック入力（`\r`/`\n`）が submit として認識されない |
| [#17603](https://github.com/anthropics/claude-code/issues/17603) | `-p` 使用時にモデルがインタラクティブモードと誤判定する |
| [#11898](https://github.com/anthropics/claude-code/issues/11898) | macOS iTerm2 で CLI がサスペンドする |
| [#17787](https://github.com/anthropics/claude-code/issues/17787) | カーソル位置応答がディスプレイに漏れる |

### 回避策ツール

#### claude-chill

- リポジトリ: [claude-chill](https://github.com/davidbeesley/claude-chill)
- PTY プロキシを作成し、VT100 ターミナルエミュレータで画面状態を追跡
- **差分アルゴリズム**で実際のターミナルへの送信データを大幅削減
- Claude Code の Ink ベースレンダラによる**フリッカーを解消**

#### Headless-TTY

- リポジトリ: [Headless-TTY](https://github.com/revoconner/Headless-TTY)
- **Windows 向けヘッドレスコンソール**（`isatty()=true` を返す）
- TTY が存在しない環境で Claude Code CLI を実行するためのワークアラウンド

#### Wake

- ブログ: [Wake - Give Claude Code Visibility Into Your Terminal History](https://dev.to/joemckenney/wake-give-claude-code-visibility-into-your-terminal-history-55o4)
- シェルを **PTY ラッパー**で包み、双方向のバイトをキャプチャ
- シェルフックと組み合わせて Claude Code にターミナル履歴の可視性を提供

#### tweakcc

- リポジトリ: [tweakcc](https://github.com/Piebald-AI/tweakcc)
- Claude Code のシステムプロンプト、ツールセット、テーマ、スピナーなどをカスタマイズ
- **70 以上のスピナーアニメーション**に対応

### デーモン/バックグラウンドプロセスの制限

- macOS は **インタラクティブな GUI プロセスにのみ TTY 権限を付与**する（デーモンプロセスは不可）
- Docker コンテナでバックグラウンドプロセスを kill すると exit code 137（SIGKILL）でクラッシュ（[#16135](https://github.com/anthropics/claude-code/issues/16135)）
- バックグラウンドエージェント（v2.0.60+）は既存のインタラクティブセッション内で並列実行可能

---

## 10. Plan モード

Claude Code にはコードベースを読み取り専用で分析し、実装計画を立てるための **Plan モード**がある。

### Plan モードへの切り替え

| 方法 | 説明 |
|------|------|
| `Shift+Tab` | セッション中にパーミッションモードを切り替え |
| `claude --permission-mode plan` | Plan モードで起動 |
| `claude --permission-mode plan -p "prompt"` | 非インタラクティブで Plan モード実行 |
| `Ctrl+G` | Plan モード中にプランをテキストエディタで編集 |

### Plan ファイルの保存場所

デフォルトでは `~/.claude/plans/` に保存される。`plansDirectory` 設定でカスタマイズ可能。

#### 設定ファイルの優先順位（高い順）

1. **Managed**（システムレベルポリシー）
2. **コマンドライン引数**
3. **Local**: `.claude/settings.local.json`（git 管理外、個人設定）
4. **Project**: `.claude/settings.json`（git 管理下、チーム共有）
5. **User**: `~/.claude/settings.json`（全プロジェクト共通）

#### 設定例

ユーザーレベル（`~/.claude/settings.json`）:

```json
{
  "plansDirectory": "~/Documents/claude-plans"
}
```

プロジェクトレベル（`.claude/settings.json`）:

```json
{
  "plansDirectory": "./project-plans"
}
```

- `~` によるホームディレクトリ展開に対応
- ディレクトリが存在しない場合は自動作成される

### opusplan モデルエイリアス

`opusplan` はプランニングと実行でモデルを使い分けるハイブリッドエイリアス:

- **Plan モード**: Opus を使用（複雑な推論・アーキテクチャ判断）
- **実行モード**: Sonnet に自動切り替え（コード生成の効率化）

---

## 参考リンク

- [CLI リファレンス](https://code.claude.com/docs/en/cli-reference)
- [ヘッドレス/プログラマティック](https://code.claude.com/docs/en/headless)
- [インタラクティブモード](https://code.claude.com/docs/en/interactive-mode)
- [ターミナル設定](https://code.claude.com/docs/en/terminal-config)
- [Hooks リファレンス](https://code.claude.com/docs/en/hooks)
- [MCP 統合](https://code.claude.com/docs/en/mcp)
- [Agent SDK 概要](https://platform.claude.com/docs/en/agent-sdk/overview)
- [ワークフロー（Plan モード）](https://code.claude.com/docs/en/common-workflows)
- [設定リファレンス](https://code.claude.com/docs/en/settings)
- [GitHub リポジトリ](https://github.com/anthropics/claude-code)
