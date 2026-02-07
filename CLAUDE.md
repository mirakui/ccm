# CCM (Claude Code Manager)

WezTerm のマルチプレクサ機能を使って複数の Claude Code セッションを管理する CLI ツール。

## プロジェクト構成

```
src/
  main.rs          -- エントリポイント、CLI ディスパッチ（new/list/switch/close）
  cli.rs           -- clap derive によるコマンド定義
  config.rs        -- 設定ファイル読み込み（~/.config/ccm/config.toml, serde + toml）
  error.rs         -- CcmError（thiserror）
  session.rs       -- Session データ構造
  state.rs         -- State ファイル管理（~/.local/state/ccm/state.json, flock + atomic write）
  wezterm.rs       -- WezTerm CLI ラッパー（spawn, split-pane, kill-pane, etc.）
  tui/
    mod.rs         -- tab-watcher TUI エントリ（panic-safe ターミナル復元）
    app.rs         -- App 状態、イベントハンドリング、reconciliation
    event.rs       -- イベントシステム（crossterm + notify + tick）
    ui.rs          -- ratatui レンダリング
```

## ビルド・実行

```bash
cargo build
cargo run -- new <session-name>
cargo run -- list
cargo run -- switch <session-name>
cargo run -- close <session-name>
```

## 設計方針

- async 不使用（WezTerm CLI は高速、TUI は同期的、file watch は別スレッド）
- Config は `main()` で1回ロードし `&Config` で各関数に渡す（グローバル変数なし）
- `wezterm.rs` には `&str` (binary) だけ渡し、config モジュールへの依存を避ける
- State は JSON ファイル + flock による排他ロック + atomic rename
- state::update() 内で読み取り→変更→書き込みを一貫して行い TOCTOU を防止
- TUI は catch_unwind でターミナル状態を必ず復元
- notify の file watcher は state.json のみにフィルタリング（.lock/.tmp を無視）

## コード品質

- コード変更後は **code-reviewer** subagent でレビューを実施すること
- CRITICAL および HIGH の指摘がゼロになるまで修正すること

## 設定ファイル

`~/.config/ccm/config.toml`（任意、なくてもデフォルト値で動作）

```toml
[wezterm]
binary = "wezterm"           # WezTerm バイナリパス
claude_command = "claude"    # claude pane に送信するコマンド（\n は自動付与）

[layout]
watcher_width = 20           # tab-watcher ペインの幅 (%, 1-99)
shell_height = 30            # shell ペインの高さ (%, 1-99)

[tui]
tick_interval_secs = 3       # reconciliation の間隔 (秒, >= 1)
```

## セッションのペインレイアウト

```
+-------------+----------------------------+
| tab-watcher |      claude code           |
|   (20%)     +----------------------------+
|             |         zsh (30%)          |
+-------------+----------------------------+
```

レイアウト比率は設定ファイルで変更可能。
