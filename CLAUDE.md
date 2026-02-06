# CCM (Claude Code Manager)

WezTerm のマルチプレクサ機能を使って複数の Claude Code セッションを管理する CLI ツール。

## プロジェクト構成

```
src/
  main.rs          -- エントリポイント、CLI ディスパッチ（new/list/switch/close）
  cli.rs           -- clap derive によるコマンド定義
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
- State は JSON ファイル + flock による排他ロック + atomic rename
- state::update() 内で読み取り→変更→書き込みを一貫して行い TOCTOU を防止
- TUI は catch_unwind でターミナル状態を必ず復元
- notify の file watcher は state.json のみにフィルタリング（.lock/.tmp を無視）

## セッションのペインレイアウト

```
+-------------+----------------------------+
| tab-watcher |      claude code (70%)     |
|   (20%)     +----------------------------+
|             |         zsh (30%)          |
+-------------+----------------------------+
```
