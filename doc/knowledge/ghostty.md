# Ghostty マルチプレクサ機能リファレンス

Ghostty はネイティブ GPU アクセラレーション対応のターミナルエミュレータであり、
tmux や Zellij のような外部マルチプレクサなしで window / tab / split (pane) を管理できる組み込みマルチプレクサ機能を持つ。

---

## 目次

1. [Window (ウィンドウ)](#1-window-ウィンドウ)
2. [Tab (タブ)](#2-tab-タブ)
3. [Split / Pane (分割ペイン)](#3-split--pane-分割ペイン)
4. [Quick Terminal (ドロップダウンターミナル)](#4-quick-terminal-ドロップダウンターミナル)
5. [キーバインドシステム](#5-キーバインドシステム)
6. [CLI 制御の現状](#6-cli-制御の現状)
7. [tmux / Zellij との機能比較](#7-tmux--zellij-との機能比較)
8. [実用的な設定例](#8-実用的な設定例)
9. [参考リンク](#9-参考リンク)

---

## 1. Window (ウィンドウ)

### デフォルトキーバインド

| アクション | Linux | macOS |
|---|---|---|
| 新規ウィンドウ | `Ctrl+Shift+N` | `Cmd+N` |
| ウィンドウを閉じる | `Alt+F4` | `Cmd+Shift+W` |
| 全ウィンドウを閉じる | -- | `all:close_window` |
| フルスクリーン切替 | `Ctrl+Enter` | `Cmd+Enter` / `Cmd+Ctrl+F` |
| アプリケーション終了 | `Ctrl+Shift+Q` | `Cmd+Q` |

### キーバインドアクション一覧

| アクション | 説明 |
|---|---|
| `new_window` | 新規ウィンドウを開く。アプリが非フォーカスの場合は前面に出す |
| `close_window` | 現在のウィンドウ (内包するタブ・スプリットすべて) を閉じる |
| `close_all_windows` | **非推奨**。`all:close_window` を使うこと |
| `toggle_maximize` | ウィンドウの最大化/解除 (Linux のみ) |
| `toggle_fullscreen` | フルスクリーン切替 |
| `toggle_window_decorations` | ウィンドウデコレーション (タイトルバー等) の切替 (Linux のみ) |
| `toggle_window_float_on_top` | 最前面固定の切替 (macOS のみ, v1.2.0+) |
| `reset_window_size` | デフォルトサイズにリセット (macOS のみ, v1.2.0+) |
| `toggle_visibility` | 全 Ghostty ウィンドウの表示/非表示 (macOS のみ) |
| `bring_all_to_front` | 全ウィンドウを前面に (macOS のみ, v1.2.0+) |

### 設定オプション

| Config Key | 型 | デフォルト | 説明 |
|---|---|---|---|
| `window-width` | integer (cells) | -- | 初期ウィンドウ幅 (セル単位)。`window-height` と対で使う |
| `window-height` | integer (cells) | -- | 初期ウィンドウ高さ (セル単位) |
| `window-position-x` | integer (px) | -- | 起動時の X 座標 (macOS のみ) |
| `window-position-y` | integer (px) | -- | 起動時の Y 座標 (macOS のみ) |
| `window-padding-x` | px/% | -- | 水平パディング。非対称指定可 (例: `2,4`) |
| `window-padding-y` | px/% | -- | 垂直パディング。非対称指定可 |
| `window-padding-balance` | bool | `true` | セル境界に合わない余白を自動分配 |
| `window-padding-color` | enum | `background` | `background` / `extend` / `extend-always` |
| `window-decoration` | enum | `auto` | `none` / `auto` / `client` / `server` |
| `window-title-font-family` | string | system | タイトルバーのフォント (GTK のみ, v1.1.0+) |
| `window-subtitle` | enum | `false` | `false` / `working-directory` (GTK のみ, v1.1.0+) |
| `window-theme` | enum | `auto` | `auto` / `system` / `light` / `dark` / `ghostty` |
| `window-colorspace` | enum | `srgb` | `srgb` / `display-p3` (macOS のみ) |
| `window-vsync` | bool | `true` | VSync 有効化 (macOS のみ) |
| `window-inherit-working-directory` | bool | -- | 新規 window/tab が前のディレクトリを引き継ぐ |
| `window-inherit-font-size` | bool | -- | 新規 window/tab がフォントサイズを引き継ぐ |
| `window-save-state` | enum | `default` | `default` / `never` / `always` (macOS のみ) |
| `window-step-resize` | bool | -- | セル単位でのリサイズ (macOS のみ) |
| `window-titlebar-background` | color | -- | タイトルバー背景色 (GTK + `window-theme=ghostty` のみ) |
| `window-titlebar-foreground` | color | -- | タイトルバー前景色 (GTK + `window-theme=ghostty` のみ) |
| `maximize` | bool | `false` | 起動時に最大化 |
| `fullscreen` | bool | `false` | 起動時にフルスクリーン |
| `initial-window` | bool | `true` | 起動時にウィンドウを作成するか |
| `quit-after-last-window-closed` | bool | macOS: `false`, Linux: `true` | 最後のウィンドウを閉じたら終了 |
| `quit-after-last-window-closed-delay` | duration | 即時 | 終了までの遅延 (Linux のみ, 最小 1s) |
| `confirm-close-surface` | enum | `true` | `true` / `false` / `always`。閉じる前に確認 |
| `title` | string | -- | 静的タイトルを強制 (エスケープシーケンスを無視) |

### 注目すべき動作

- **Undo/Redo** (macOS, v1.2.0+): ウィンドウ/タブ/スプリットを閉じた後 `Cmd+Z` で復元、`Cmd+Shift+Z` でやり直し。`undo-timeout` (デフォルト 5 秒) で保持期間を設定
- **状態の永続化** (`window-save-state = always`): macOS でタブと作業ディレクトリが再起動後も復元される。ただしスプリットのレイアウトは完全には復元されないとの報告あり

---

## 2. Tab (タブ)

### デフォルトキーバインド

| アクション | Linux | macOS |
|---|---|---|
| 新規タブ | `Ctrl+Shift+T` | `Cmd+T` |
| タブ/サーフェスを閉じる | `Ctrl+Shift+W` | `Cmd+W` |
| 前のタブ | `Ctrl+Shift+Tab` / `Ctrl+Shift+Left` / `Ctrl+Page Up` | `Cmd+Shift+[` |
| 次のタブ | `Ctrl+Tab` / `Ctrl+Shift+Right` / `Ctrl+Page Down` | `Cmd+Shift+]` |
| タブ 1-8 に移動 | `Alt+[1-8]` | `Cmd+[1-8]` |
| 最後のタブ | `Alt+9` | `Cmd+9` |

### キーバインドアクション一覧

| アクション | 説明 |
|---|---|
| `new_tab` | 新規タブを開く |
| `close_tab` | 現在のタブ (内包するスプリットすべて) を閉じる |
| `close_tab:other` | 現在のタブ以外をすべて閉じる (v1.2.0+) |
| `previous_tab` | 前のタブに移動 |
| `next_tab` | 次のタブに移動 |
| `last_tab` | 最後に使用したタブに移動 |
| `goto_tab:<N>` | N 番目のタブに移動 (1-based)。存在しない番号は最後のタブに移動 |
| `move_tab:<offset>` | タブを相対位置で移動。`move_tab:1` で右へ、`move_tab:-1` で左へ。循環する |
| `toggle_tab_overview` | タブ一覧表示の切替 (Linux のみ, libadwaita 1.4+) |
| `prompt_surface_title` | タブタイトルの変更ダイアログ (Linux のみ, v1.2.0+) |

### 設定オプション

| Config Key | 型 | デフォルト | 説明 |
|---|---|---|---|
| `window-new-tab-position` | enum | `current` | `current` / `end`。新規タブの挿入位置 |
| `window-show-tab-bar` | enum | `auto` | `always` / `auto` / `never` (GTK のみ, v1.2.0+) |

### 注目すべき動作

- **タイトルバー内タブ** (GTK, v1.2.0+): `gtk-titlebar-style=tabs` でタブをタイトルバーに統合し、省スペース化
- **新規タブボタンのドロップダウン** (GTK, v1.2.0+): 新規タブボタンからスプリットも作成可能
- タブは **プラットフォームネイティブ UI** を使用 (macOS ネイティブタブバー、GTK タブバー)

---

## 3. Split / Pane (分割ペイン)

### デフォルトキーバインド

| アクション | Linux | macOS |
|---|---|---|
| 右に分割 (垂直分割線) | `Ctrl+Shift+O` | `Cmd+D` |
| 下に分割 (水平分割線) | `Ctrl+Shift+E` | `Cmd+Shift+D` |
| 前のスプリットへ | `Ctrl+Super+[` | `Cmd+[` |
| 次のスプリットへ | `Ctrl+Super+]` | `Cmd+]` |
| 上のスプリットへ | `Ctrl+Alt+Up` | `Cmd+Option+Up` |
| 下のスプリットへ | `Ctrl+Alt+Down` | `Cmd+Option+Down` |
| 左のスプリットへ | `Ctrl+Alt+Left` | `Cmd+Option+Left` |
| 右のスプリットへ | `Ctrl+Alt+Right` | `Cmd+Option+Right` |
| スプリットズーム切替 | `Ctrl+Shift+Enter` | `Cmd+Shift+Enter` |
| 上方向にリサイズ | `Ctrl+Super+Shift+Up` | `Cmd+Ctrl+Up` |
| 下方向にリサイズ | `Ctrl+Super+Shift+Down` | `Cmd+Ctrl+Down` |
| 左方向にリサイズ | `Ctrl+Super+Shift+Left` | `Cmd+Ctrl+Left` |
| 右方向にリサイズ | `Ctrl+Super+Shift+Right` | `Cmd+Ctrl+Right` |
| スプリット均等化 | `Ctrl+Super+Shift+=` | `Cmd+Ctrl+=` |

### キーバインドアクション一覧

#### `new_split:<direction>` - 新規スプリット作成

| direction | 説明 |
|---|---|
| `right` | 右方向に分割 (垂直分割線を追加) |
| `down` | 下方向に分割 (水平分割線を追加) |
| `left` | 左方向に分割 |
| `up` | 上方向に分割 |
| `auto` | 現在のペインの大きい方の次元で自動判定。幅 > 高さなら right、高さ > 幅なら down |

#### `goto_split:<target>` - スプリット間ナビゲーション

| target | 説明 |
|---|---|
| `left` / `right` / `up` / `down` | 方向指定ナビゲーション。v1.2.0+ で空間ナビゲーション (最近接ペインを選択) |
| `previous` / `next` | 作成順でのナビゲーション |

`performable:` プレフィックスと組み合わせると、移動先がない場合はキー入力を消費しない (v1.2.0+)。

#### その他のスプリットアクション

| アクション | 説明 |
|---|---|
| `toggle_split_zoom` | 現在のスプリットをタブ全体に拡大/復元。ズーム中はタブバーにアイコン表示 |
| `resize_split:<direction>,<pixels>` | 指定方向に指定ピクセル分リサイズ。例: `resize_split:up,10` |
| `equalize_splits` | 全スプリットのサイズを均等化。v1.2.0 で複数同方向スプリットの挙動改善 |
| `close_surface` | 現在のサーフェス (window/tab/split) を閉じる汎用アクション |

### 設定オプション

| Config Key | 型 | デフォルト | 説明 |
|---|---|---|---|
| `unfocused-split-opacity` | float | `1.0` | 非フォーカスペインの不透明度 (0.15 〜 1.0) |
| `unfocused-split-fill` | color | background色 | 非フォーカスペインのオーバーレイ色 |
| `split-divider-color` | color | 自動 | 分割線の色 (v1.1.0+) |
| `focus-follows-mouse` | bool | `false` | マウスホバーでペインをフォーカス |

### スプリットの動作詳細

- **ツリー構造レイアウト**: スプリットはネストしたツリー構造。各分割で 2 つの子ペインが生成される。tmux のペイン管理と類似のモデル
- **マウスによるリサイズ**: 分割線をドラッグしてリサイズ可能
- **タブ間ナビゲーション不可**: `goto_split` は現在のタブ内でのみ動作。タブ間のスプリットナビゲーションは未対応 (feature request: #9031)
- **メニューからの全方向分割** (v1.2.0+): macOS と GTK の両方でメニューバー/コンテキストメニューから 4 方向すべての分割が可能

---

## 4. Quick Terminal (ドロップダウンターミナル)

Quake スタイルのドロップダウンターミナル。グローバルキーバインドで呼び出し/非表示を切替。
状態は show/hide 間で保持される。

### キーバインド設定例

```
keybind = global:ctrl+grave_accent=toggle_quick_terminal
```

macOS の場合:

```
keybind = global:cmd+backquote=toggle_quick_terminal
```

`global:` プレフィックスが必須 (Ghostty 非フォーカス時にも動作させるため)。

### 設定オプション

| Config Key | 型 | デフォルト | 説明 |
|---|---|---|---|
| `quick-terminal-position` | enum | -- | `top` / `bottom` / `left` / `right` / `center` |
| `quick-terminal-size` | string | -- | サイズ (`20%` やピクセル `300px` で指定) |
| `quick-terminal-screen` | enum | `main` | `main` / `mouse` / `macos-menu-bar` (macOS のみ) |
| `quick-terminal-animation-duration` | float (秒) | -- | アニメーション時間。0 で無効 (macOS のみ) |
| `quick-terminal-autohide` | bool | macOS: `true`, Linux: `false` | フォーカスが外れたら自動非表示 |
| `quick-terminal-space-behavior` | enum | `move` | `move` / `remain` (macOS Spaces 切替時の挙動, v1.1.0+) |
| `quick-terminal-keyboard-interactivity` | enum | `on-demand` | `none` / `on-demand` / `exclusive` (Wayland のみ, v1.2.0+) |

### 注意事項

- **macOS**: グローバルキーバインドには Accessibility 権限が必要 (システム設定 > プライバシーとセキュリティ > アクセシビリティ)
- **Linux**: Wayland のみ対応 (`wlr-layer-shell-v1` プロトコル対応 Compositor が必要, v1.2.0+)
- Quick Terminal 内で **タブとスプリットが完全に動作** (v1.2.0+)

---

## 5. キーバインドシステム

### 構文

```
keybind = [prefix:]trigger=action[:parameter]
```

### 修飾キー

| 修飾キー | エイリアス |
|---|---|
| `ctrl` | `control` |
| `alt` | `opt`, `option` |
| `super` | `cmd`, `command` |
| `shift` | -- |

複数の修飾キーは `+` で結合: `ctrl+shift+t`

### キーシーケンス (リーダーキー)

`>` でシーケンスを区切る。最初のキーを押して離し、次のキーを押す。

```
keybind = ctrl+a>d=close_surface
```

`Ctrl+A` を押して離し、`d` を押すと `close_surface` が実行される。

### プレフィックス

| プレフィックス | 説明 |
|---|---|
| `all:` | 全サーフェスに適用 (フォーカス中のものだけでなく) |
| `global:` | システム全体で動作 (Ghostty 非フォーカス時も)。`all:` を暗黙的に含む |
| `unconsumed:` | キー入力を消費せず、実行中プログラムにも渡す |
| `performable:` | アクションが実行可能な場合のみ消費。例: 選択範囲がないときの `copy_to_clipboard` はキーをスルー (v1.1.0+) |

プレフィックスは組み合わせ可能: `performable:ctrl+c=copy_to_clipboard`

### 特殊アクション

| アクション | 説明 |
|---|---|
| `unbind` | バインドを解除。キーが印字可能なら子プロセスに送信 |
| `ignore` | 入力を無視 |
| `text:<string>` | 任意テキスト送信 (Zig リテラル構文: `text:\x15` で Ctrl-U) |
| `csi:<text>` | CSI シーケンスを送信 |
| `esc:<text>` | エスケープシーケンスを送信 |

### デフォルトバインドの全消去

```
keybind = clear
```

外部マルチプレクサ (tmux, Zellij) との競合を避けるために有用。

### 便利なコマンド

```bash
# デフォルトキーバインド一覧
ghostty +list-keybinds --default

# 利用可能なアクション一覧
ghostty +list-actions
```

**コマンドパレット** (v1.2.0+): `Ctrl+Shift+P` (GTK) / `Cmd+Shift+P` (macOS) でインタラクティブにアクションを検索・実行。

---

## 6. CLI 制御の現状

### 概要

Ghostty には `ghostty +<command>` 形式の CLI サブコマンドがいくつか存在するが、
**タブ切替・ペイン操作・ウィンドウ制御などをプログラムから行う CLI / IPC API は現時点で存在しない。**

`next_tab`, `goto_tab:<N>`, `new_split:right` などのアクションはすべて **キーバインド経由でのみ** 発火可能であり、
tmux の `select-window -t` や Zellij の `zellij action go-to-tab` に相当するコマンドはない。

### 利用可能な CLI サブコマンド

```bash
ghostty +list-keybinds          # 現在のキーバインド一覧
ghostty +list-keybinds --default # デフォルトキーバインド一覧
ghostty +list-actions           # 利用可能なキーバインドアクション一覧
ghostty +list-themes            # 利用可能なテーマ一覧
ghostty +list-fonts             # 利用可能なフォント一覧
ghostty +list-colors            # 現在のカラーパレット
ghostty +show-config            # 現在の設定を表示
ghostty +validate-config        # 設定ファイルのバリデーション
ghostty +crash-report           # 直近のクラッシュレポートを表示
```

これらはすべて **情報取得用** であり、実行中の Ghostty インスタンスを制御するものではない。

### Scripting API / IPC の検討状況

Ghostty の Scripting API は [Discussion #2353](https://github.com/ghostty-org/ghostty/discussions/2353) で議論されているが、未実装でタイムラインも未定。

#### 検討中のアプローチ

| アプローチ | 説明 | 状況 |
|---|---|---|
| **プラットフォーム固有 IPC** | macOS: AppleScript / App Intents、Linux: D-Bus | 優先度高だが未完成。D-Bus は部分的に実装あり |
| **制御シーケンス (Control Sequences)** | Kitty protocol や tmux control mode のように、TUI アプリが Ghostty を直接制御 | セキュリティ設計が必要で停滞中 |
| **Unix ドメインソケット** | クロスプラットフォームなテキストプロトコル (memcached/redis 風) | 初期案だがプラットフォーム固有 IPC 優先に方針転換 |

メンテナ (mitchellh) はセキュリティ上の懸念 (悪意あるエスケープシーケンスの防止) と API スコープの管理を重視しており、
統一的な API よりもプラットフォーム固有の狭いスコープで段階的に実装する方針を示している。

### 関連する未解決の要望

| Discussion | 内容 |
|---|---|
| [#2353](https://github.com/ghostty-org/ghostty/discussions/2353) | Scripting API 全般 |
| [#3782](https://github.com/ghostty-org/ghostty/discussions/3782) | CLI からタブ/ウィンドウの一覧取得 (回答: #2353 が必要) |
| [#4579](https://github.com/ghostty-org/ghostty/discussions/4579) | CLI から既存インスタンスに新規タブを開く |
| [#2480](https://github.com/ghostty-org/ghostty/discussions/2480) | 起動時のスプリットレイアウト定義 |
| [#5912](https://github.com/ghostty-org/ghostty/discussions/5912) | デフォルトスプリットレイアウト |
| [#3358](https://github.com/ghostty-org/ghostty/discussions/3358) | セッションマネージャ |

### 現時点でのワークアラウンド

#### macOS: AppleScript による間接操作

```applescript
-- Ghostty のウィンドウにキーストロークを送信
tell application "System Events"
    tell process "Ghostty"
        keystroke "t" using command down  -- Cmd+T で新規タブ
        keystroke "2" using command down  -- Cmd+2 でタブ 2 に移動
    end tell
end tell
```

#### Linux: xdotool / ydotool によるキーシミュレーション

```bash
# xdotool でキーストロークを送信 (X11)
xdotool key --window $(xdotool search --name "Ghostty") ctrl+shift+t

# ydotool (Wayland)
ydotool key ctrl+shift+t
```

#### 汎用: プロセスシグナル

Ghostty はシグナルベースの制御は公式にはサポートしていないが、
`SIGUSR1` でコンフィグのリロードが可能:

```bash
kill -SIGUSR1 $(pgrep ghostty)
```

### tmux / Zellij の CLI 制御との比較

| 操作 | tmux | Zellij | Ghostty |
|---|---|---|---|
| タブ切替 | `tmux select-window -t N` | `zellij action go-to-tab N` | **不可** |
| ペイン切替 | `tmux select-pane -t N` | `zellij action focus-next-pane` | **不可** |
| 新規タブ | `tmux new-window` | `zellij action new-tab` | **不可** |
| 新規スプリット | `tmux split-window` | `zellij action new-pane` | **不可** |
| タブ一覧 | `tmux list-windows` | `zellij action query-tab-names` | **不可** |
| レイアウト適用 | `tmux select-layout` | `zellij action dump-layout` | **不可** |
| コマンド送信 | `tmux send-keys` | `zellij action write` | **不可** |

CLI からの制御が必要なワークフロー (自動化スクリプト、セッション管理、IDE 連携など) では、
引き続き tmux / Zellij が必須となる。

---

## 7. tmux / Zellij との機能比較

### Ghostty が提供する機能 (tmux/Zellij と重複)

| 機能 | Ghostty | tmux | Zellij |
|---|---|---|---|
| 複数ウィンドウ | OS ネイティブウィンドウ | 単一ターミナル内 | 単一ターミナル内 |
| タブ | OS ネイティブタブ | window として | tab として |
| スプリット / ペイン | ネイティブ GPU レンダリング | あり | あり |
| ペインズーム | `toggle_split_zoom` | `zoom-pane` | あり |
| リサイズ | キーボード + マウスドラッグ | あり | あり |
| 均等分割 | `equalize_splits` | `select-layout even-*` | あり |
| フォーカスフォローマウス | `focus-follows-mouse` | `mouse-select-pane` | あり |
| リーダーキー / シーケンス | `ctrl+a>...` 形式 | prefix キー | モード切替 |
| タブ移動 | `move_tab` | あり | あり |
| ドロップダウンターミナル | Quick Terminal | なし | なし (外部ツール必要) |
| コマンドパレット | v1.2.0+ | なし | なし |
| 閉じた操作の Undo | macOS v1.2.0+ | なし | なし |

### Ghostty にない機能

| 機能 | 説明 |
|---|---|
| **セッション永続化 (detach/attach)** | **最大の制限事項**。Ghostty にはセッション概念がなく、ターミナルを閉じるとプロセスが終了する。tmux/Zellij の最重要機能 |
| **定義済みレイアウト** | 起動時のスプリットレイアウト定義は不可。頻繁に要望されているが未実装 (Discussion #2480, #5912) |
| **スクリプタブルなレイアウト作成** | tmuxinator や Zellij のレイアウトファイルに相当する機能なし |
| **リモートセッション管理** | SSH 越しのセッション attach は不可。ローカルのみ |
| **コピーモード (vi/emacs キー)** | tmux のようなモーダルなスクロールバック検索なし |
| **セッション共有** | 複数クライアントの同一セッション接続不可 |
| **ペイン同期** | `synchronize-panes` 相当なし |
| **タブ間ペインナビゲーション** | `goto_split` はタブ内限定 (feature request: #9031) |
| **カスタムステータスバー** | OS ネイティブ UI のみ |

### 使い分けガイド

- **セッション永続化が必要** (長時間プロセス、SSH リモート作業) → tmux / Zellij が必須
- **ローカル作業のみで splits/tabs/windows だけ使う** → Ghostty 単体で tmux/Zellij を完全に置換可能。ネイティブ GPU レンダリングと簡潔な設定が利点
- **ハイブリッド構成**: Ghostty の window/tab 層と tmux/Zellij のセッション管理を併用。キーバインド競合を避けるため `keybind = clear` で Ghostty のデフォルトを消去し、必要なものだけ再定義

---

## 8. 実用的な設定例

### Vim スタイルのスプリットナビゲーション

```
keybind = opt+h=goto_split:left
keybind = opt+j=goto_split:down
keybind = opt+k=goto_split:up
keybind = opt+l=goto_split:right
```

### tmux スタイル (Ctrl+A リーダーキー)

```
keybind = ctrl+a>h=new_split:left
keybind = ctrl+a>j=new_split:down
keybind = ctrl+a>k=new_split:up
keybind = ctrl+a>l=new_split:right
keybind = ctrl+a>f=toggle_split_zoom
keybind = ctrl+a>n=next_tab
keybind = ctrl+a>p=previous_tab
keybind = ctrl+a>c=new_tab
keybind = ctrl+a>d=close_surface
```

### Emacs スタイル (キーシーケンス)

```
keybind = ctrl+x>2=new_split:right
keybind = ctrl+x>3=new_split:up
```

### 非フォーカスペインの視覚的区別

```
unfocused-split-opacity = 0.35
unfocused-split-fill = ffc0cb
split-divider-color = #91d7e3
```

### 外部マルチプレクサとの併用 (tmux/Zellij 優先)

```
# デフォルトキーバインドをすべて消去
keybind = clear

# 最低限のキーバインドだけ残す
keybind = cmd+n=new_window
keybind = cmd+w=close_surface
keybind = cmd+q=quit
keybind = cmd+plus=increase_font_size:1
keybind = cmd+minus=decrease_font_size:1
keybind = cmd+zero=reset_font_size
```

---

## 9. 参考リンク

- [Ghostty 公式ドキュメント - Features](https://ghostty.org/docs/features)
- [Ghostty キーバインドアクションリファレンス](https://ghostty.org/docs/config/keybind/reference)
- [Ghostty キーバインド設定](https://ghostty.org/docs/config/keybind)
- [Ghostty 設定リファレンス](https://ghostty.org/docs/config/reference)
- [Ghostty キーシーケンス](https://ghostty.org/docs/config/keybind/sequence)
- [Ghostty v1.1.0 リリースノート](https://ghostty.org/docs/install/release-notes/1-1-0)
- [Ghostty v1.2.0 リリースノート](https://ghostty.org/docs/install/release-notes/1-2-0)
- [Replacing tmux with Ghostty (sterba.dev)](https://sterba.dev/posts/replacing-tmux/)
- [Ghostty Split Layout Discussion (#2480)](https://github.com/ghostty-org/ghostty/discussions/2480)
- [Ghostty Session Manager Discussion (#3358)](https://github.com/ghostty-org/ghostty/discussions/3358)
- [Ghostty GitHub リポジトリ](https://github.com/ghostty-org/ghostty)
