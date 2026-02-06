# WezTerm マルチプレクサ機能リファレンス

WezTerm は GPU アクセラレーション対応のターミナルエミュレータであり、tmux や screen のようなターミナルマルチプレクサ機能を内蔵している。Lua による設定が可能で、Window / Tab / Pane の階層構造によりターミナルセッションを管理する。

---

## 目次

1. [階層構造の概要](#1-階層構造の概要)
2. [Window 管理](#2-window-管理)
3. [Tab 管理](#3-tab-管理)
4. [Pane 管理](#4-pane-管理)
5. [マルチプレクサアーキテクチャ](#5-マルチプレクサアーキテクチャ)
6. [キーバインド](#6-キーバインド)
7. [Lua API リファレンス](#7-lua-api-リファレンス)
8. [Pane オブジェクトとメソッド](#8-pane-オブジェクトとメソッド)
9. [高度な機能](#9-高度な機能)
10. [リモートマルチプレクシング](#10-リモートマルチプレクシング)
11. [設定例](#11-設定例)

---

## 1. 階層構造の概要

WezTerm のマルチプレクサは以下の階層で構成される:

```
Workspace
  └── MuxWindow (GUI ウィンドウ)
        └── Tab (タブ)
              └── Pane (ペイン / ターミナル分割領域)
```

- **Workspace**: MuxWindow をグループ化するラベル。tmux のセッションに相当する。GUI は一度にひとつの Workspace のみ表示する
- **MuxWindow**: OS のウィンドウに対応。複数の Tab を持つ
- **Tab**: ウィンドウ内のタブ。複数の Pane を持つ
- **Pane**: 個々のターミナルインスタンス。分割によって増やせる

---

## 2. Window 管理

### 2.1 Window の作成

| アクション | 説明 |
|-----------|------|
| `SpawnWindow` | デフォルトドメインで新しいウィンドウを作成 |
| `SpawnCommandInNewWindow` | 指定コマンドで新しいウィンドウを作成 |

**デフォルトキーバインド:**

- `Super+N` / `Ctrl+Shift+N` — 新規ウィンドウ

**Lua 設定:**

```lua
-- 単純な新規ウィンドウ
{ key = 'n', mods = 'SHIFT|CTRL', action = wezterm.action.SpawnWindow }

-- コマンド指定で新規ウィンドウ
{ key = 'y', mods = 'CMD', action = wezterm.action.SpawnCommandInNewWindow {
    args = { 'top' },
    cwd = '/tmp',
    set_environment_variables = { FOO = 'BAR' },
  },
}
```

**プログラム的な作成 (`wezterm.mux.spawn_window`):**

```lua
-- 戻り値は (MuxTab, Pane, MuxWindow) のタプル
local tab, pane, window = wezterm.mux.spawn_window {
  args = { 'top' },
  cwd = '/tmp',
  set_environment_variables = { FOO = 'BAR' },
  domain = { DomainName = 'my.name' },  -- または 'DefaultDomain'
  width = 80,       -- 列数（文字セル単位）
  height = 24,      -- 行数（文字セル単位）
  workspace = 'coding',
  position = {      -- v20230320 以降
    x = 100,
    y = 100,
    origin = 'ActiveScreen',
  },
}
```

### 2.2 Window の切り替え

| アクション | 説明 |
|-----------|------|
| `ActivateWindow(n)` | n 番目の GUI ウィンドウをアクティブにする（0始まり） |
| `ActivateWindowRelative(delta)` | ウィンドウを巡回（ラップあり） |
| `ActivateWindowRelativeNoWrap(delta)` | ウィンドウを巡回（ラップなし） |

```lua
-- CMD+ALT+数字でウィンドウ切り替え
for i = 1, 8 do
  table.insert(config.keys, {
    key = tostring(i), mods = 'CMD|ALT', action = act.ActivateWindow(i - 1),
  })
end

-- ウィンドウ巡回
{ key = 'r', mods = 'ALT', action = act.ActivateWindowRelative(1) },
{ key = 'e', mods = 'ALT', action = act.ActivateWindowRelative(-1) },
```

### 2.3 その他の Window アクション

| アクション | デフォルトバインド | 説明 |
|-----------|------------------|------|
| `Hide` | `Super+M` | ウィンドウを隠す |
| `HideApplication` | `Super+H` (macOS) | アプリケーション全体を隠す |
| `ToggleFullScreen` | `Alt+Enter` | フルスクリーン切り替え |
| `ToggleAlwaysOnTop` | なし | 常に最前面に表示 |
| `ToggleAlwaysOnBottom` | なし | 常に最背面に表示 |
| `SetWindowLevel` | なし | ウィンドウレベルを明示的に設定 |
| `StartWindowDrag` | なし | ウィンドウドラッグを開始 |
| `QuitApplication` | なし | WezTerm を終了 |

### 2.4 MuxWindow オブジェクトメソッド

| メソッド | 説明 |
|---------|------|
| `window:active_pane()` | 現在アクティブな Pane を取得 |
| `window:active_tab()` | 現在アクティブな Tab を取得 |
| `window:get_title()` | ウィンドウタイトルを取得 |
| `window:set_title(title)` | ウィンドウタイトルを設定 |
| `window:get_workspace()` | 所属する Workspace を取得 |
| `window:set_workspace(name)` | Workspace を変更 |
| `window:spawn_tab{}` | 新しい Tab を作成 |
| `window:tabs()` | 全 Tab を一覧 |
| `window:tabs_with_info()` | メタデータ付きで全 Tab を一覧 |
| `window:window_id()` | ウィンドウの一意識別子を取得 |
| `window:gui_window()` | GUI 表現にアクセス |

---

## 3. Tab 管理

### 3.1 Tab の作成

| アクション | 説明 |
|-----------|------|
| `SpawnTab 'CurrentPaneDomain'` | アクティブ Pane と同じドメインで新規 Tab |
| `SpawnTab 'DefaultDomain'` | デフォルトドメインで新規 Tab |
| `SpawnTab { DomainName = 'unix' }` | 指定ドメインで新規 Tab |
| `SpawnCommandInNewTab { ... }` | コマンド指定で新規 Tab |

**デフォルトキーバインド:**

- `Super+T` / `Ctrl+Shift+T` — 新規 Tab（現在の Pane ドメイン）
- `Super+Shift+T` — 新規 Tab（デフォルトドメイン）

```lua
-- 基本的な Tab 作成
{ key = 't', mods = 'SHIFT|ALT', action = act.SpawnTab 'CurrentPaneDomain' }

-- 特定ドメインで Tab 作成
{ key = 't', mods = 'SHIFT|ALT', action = act.SpawnTab { DomainName = 'unix' } }

-- コマンド指定で Tab 作成
{ key = 'y', mods = 'CMD', action = act.SpawnCommandInNewTab {
    args = { 'top' },
    cwd = '/some/path',
    set_environment_variables = { TERM_TYPE = 'monitoring' },
  },
}
```

**プログラム的な作成:**

```lua
local tab, pane, window = window:spawn_tab {
  args = { 'top' },
  cwd = '/tmp',
  set_environment_variables = { FOO = 'BAR' },
  domain = { DomainName = 'my.name' },
}
```

### 3.2 Tab の切り替え

| アクション | デフォルトバインド | 説明 |
|-----------|------------------|------|
| `ActivateTab(n)` | `Super+1`..`9` / `Ctrl+Shift+1`..`9` | インデックス指定で切り替え（0始まり、負数でラップ: -1 = 最後） |
| `ActivateTabRelative(offset)` | `Super+Shift+]` / `Ctrl+Tab` / `Ctrl+PageDown` (+1), `Super+Shift+[` / `Ctrl+Shift+Tab` / `Ctrl+PageUp` (-1) | 相対的に切り替え（ラップあり） |
| `ActivateTabRelativeNoWrap(offset)` | なし | 相対的に切り替え（ラップなし） |
| `ActivateLastTab` | なし | 直前にアクティブだった Tab に切り替え |

```lua
-- インデックス指定
for i = 1, 8 do
  table.insert(config.keys, {
    key = tostring(i), mods = 'CTRL|ALT', action = act.ActivateTab(i - 1),
  })
end

-- 相対ナビゲーション
{ key = '{', mods = 'ALT', action = act.ActivateTabRelative(-1) },
{ key = '}', mods = 'ALT', action = act.ActivateTabRelative(1) },

-- 最後の Tab（tmux スタイルの Leader キー付き）
config.leader = { key = 'a', mods = 'CTRL' }
{ key = 'o', mods = 'LEADER|CTRL', action = act.ActivateLastTab },
```

### 3.3 Tab の移動

| アクション | デフォルトバインド | 説明 |
|-----------|------------------|------|
| `MoveTab(index)` | なし | Tab を絶対位置に移動 |
| `MoveTabRelative(offset)` | `Ctrl+Shift+PageUp` (-1) / `Ctrl+Shift+PageDown` (+1) | Tab を左右に移動 |

```lua
for i = 1, 8 do
  table.insert(config.keys, {
    key = tostring(i), mods = 'CTRL|ALT', action = act.MoveTab(i - 1),
  })
end
```

### 3.4 Tab のクローズ

| アクション | デフォルトバインド | 説明 |
|-----------|------------------|------|
| `CloseCurrentTab { confirm = true }` | `Super+W` / `Ctrl+Shift+W` | Tab を閉じる（確認ダイアログ付き） |

`confirm` パラメータが `true` の場合、確認オーバーレイが表示される。`skip_close_confirmation_for_processes_named` で確認をスキップするプロセスをホワイトリスト登録できる。

### 3.5 Tab のリネーム

`PromptInputLine` と `set_title()` を組み合わせて実現する:

```lua
{ key = 'E', mods = 'CTRL|SHIFT',
  action = act.PromptInputLine {
    description = 'Enter new tab name',
    action = wezterm.action_callback(function(window, pane, line)
      if line then
        window:active_tab():set_title(line)
      end
    end),
  },
}
```

### 3.6 MuxTab オブジェクトメソッド

| メソッド | 説明 |
|---------|------|
| `tab:activate()` | Tab をアクティブにする |
| `tab:tab_id()` | Tab の識別子を取得 |
| `tab:window()` | 親の MuxWindow を取得 |
| `tab:active_pane()` | アクティブな Pane を取得 |
| `tab:panes()` | 全 Pane を取得 |
| `tab:panes_with_info()` | メタデータ付き（PaneInformation）で全 Pane を取得 |
| `tab:get_pane_direction(direction)` | 指定方向の Pane を検索 |
| `tab:rotate_clockwise()` | Pane レイアウトを時計回りに回転 |
| `tab:rotate_counter_clockwise()` | Pane レイアウトを反時計回りに回転 |
| `tab:set_title(title)` | Tab タイトルを設定 |
| `tab:get_title()` | Tab タイトルを取得 |
| `tab:set_zoomed(bool)` | ズーム状態をプログラム的に制御 |
| `tab:get_size()` | Tab のサイズを取得 |

---

## 4. Pane 管理

### 4.1 Pane の分割

3 つの主要なアクションがある:

#### SplitPane（最も柔軟、v20220624 以降）

```lua
{ key = '%', mods = 'CTRL|SHIFT|ALT',
  action = act.SplitPane {
    direction = 'Left',     -- 'Up', 'Down', 'Left', 'Right'
    command = { args = { 'top' } },  -- オプション: SpawnCommand
    size = { Percent = 50 },         -- または { Cells = 10 }
    top_level = false,               -- true = Tab 全体を分割（Pane 単位ではなく）
  },
}
```

#### SplitHorizontal（左右分割）

```lua
-- デフォルト: 現在の Pane が左、新しい Pane が右
{ key = '%', mods = 'CTRL|SHIFT|ALT',
  action = act.SplitHorizontal { domain = 'CurrentPaneDomain' },
}

-- コマンド指定:
{ key = '%', mods = 'CTRL|SHIFT|ALT',
  action = act.SplitHorizontal { args = { 'top' } },
}
```

#### SplitVertical（上下分割）

```lua
-- デフォルト: 現在の Pane が上、新しい Pane が下
{ key = '"', mods = 'CTRL|SHIFT|ALT',
  action = act.SplitVertical { domain = 'CurrentPaneDomain' },
}
```

**デフォルトキーバインド:**

- `Ctrl+Shift+Alt+"` — 上下分割
- `Ctrl+Shift+Alt+%` — 左右分割

**プログラム的な分割 (`pane:split{}`):**

```lua
local new_pane = pane:split {
  args = { 'top' },
  cwd = '/tmp',
  set_environment_variables = { FOO = 'BAR' },
  domain = 'CurrentPaneDomain',    -- または 'DefaultDomain', { DomainName = '...' }
  direction = 'Right',             -- 'Right'（デフォルト）, 'Left', 'Top', 'Bottom'
  top_level = false,               -- true = Tab ルートで分割
  size = 0.5,                      -- <1.0 = 割合、>=1 = セル数
}
```

### 4.2 Pane のナビゲーション

#### ActivatePaneDirection

方向: `'Left'`, `'Right'`, `'Up'`, `'Down'`, `'Next'`（v20220101 以降）, `'Prev'`（v20220101 以降）

同じ方向に複数の Pane がある場合、最も最近アクティブだった Pane が選択される（v20220903 以降）。

**デフォルトバインド:** `Ctrl+Shift+矢印キー`

```lua
{ key = 'LeftArrow',  mods = 'CTRL|SHIFT', action = act.ActivatePaneDirection 'Left' },
{ key = 'RightArrow', mods = 'CTRL|SHIFT', action = act.ActivatePaneDirection 'Right' },
{ key = 'UpArrow',    mods = 'CTRL|SHIFT', action = act.ActivatePaneDirection 'Up' },
{ key = 'DownArrow',  mods = 'CTRL|SHIFT', action = act.ActivatePaneDirection 'Down' },
```

関連設定: `unzoom_on_switch_pane`（デフォルト: `true`）— `true` の場合、ズーム中に `ActivatePaneDirection` で切り替えると自動的にアンズームする。`false` の場合はズーム中の切り替えがブロックされる。

#### ActivatePaneByIndex

```lua
{ key = '3', mods = 'ALT', action = act.ActivatePaneByIndex(2) }
```

#### PaneSelect

モーダルオーバーレイで各 Pane にラベルを表示し、ラベルを入力して選択する。

モード:

| モード | 説明 |
|-------|------|
| `"Activate"` | フォーカスを移す（デフォルト） |
| `"SwapWithActive"` | アクティブ Pane と位置を入れ替え、フォーカスも移動 |
| `"SwapWithActiveKeepFocus"` | 位置を入れ替えるが、フォーカスは元のまま（v20240127） |
| `"MoveToNewTab"` | 選択した Pane を新しい Tab に移動（v20240127） |
| `"MoveToNewWindow"` | 選択した Pane を新しい Window に移動（v20240127） |

```lua
{ key = '8', mods = 'CTRL', action = act.PaneSelect },
{ key = '9', mods = 'CTRL', action = act.PaneSelect { alphabet = '1234567890' } },
{ key = '0', mods = 'CTRL', action = act.PaneSelect {
    mode = 'SwapWithActive',
    show_pane_ids = true,   -- v20240127 以降
  },
},
```

設定: `pane_select_font_size`（デフォルト: 36）

### 4.3 Pane のリサイズ

**`AdjustPaneSize { direction, amount }`:**

**デフォルトバインド:** `Ctrl+Shift+Alt+矢印キー`

```lua
{ key = 'H', mods = 'LEADER', action = act.AdjustPaneSize { 'Left', 5 } },
{ key = 'J', mods = 'LEADER', action = act.AdjustPaneSize { 'Down', 5 } },
{ key = 'K', mods = 'LEADER', action = act.AdjustPaneSize { 'Up', 5 } },
{ key = 'L', mods = 'LEADER', action = act.AdjustPaneSize { 'Right', 5 } },
```

### 4.4 Pane の回転

**`RotatePanes(direction)`:** Pane の内容を回転させる。サイズ配分は維持される。

方向: `'Clockwise'`, `'CounterClockwise'`

Pane が [0, 1, 2] の場合:
- Clockwise: [2, 0, 1]
- CounterClockwise: [1, 2, 0]

```lua
{ key = 'b', mods = 'CTRL', action = act.RotatePanes 'CounterClockwise' },
{ key = 'n', mods = 'CTRL', action = act.RotatePanes 'Clockwise' },
```

### 4.5 Pane のズーム

| アクション | 説明 |
|-----------|------|
| `TogglePaneZoomState` | ズームの切り替え（デフォルト: `Ctrl+Shift+Z`） |
| `SetPaneZoomState(true/false)` | 明示的にズーム/アンズーム（v20220807 以降） |

ズーム中の Pane は Tab 全体のスペースを占有し、他の Pane は非表示になる。アンズームすると元のレイアウトに戻る。

```lua
{ key = 'Z', mods = 'CTRL', action = act.TogglePaneZoomState },
{ key = 'Z', mods = 'CTRL|SHIFT', action = act.SetPaneZoomState(true) },
```

プログラム的: `tab:set_zoomed(bool)`

### 4.6 Pane のクローズ

**`CloseCurrentPane { confirm = bool }`:**

`confirm = true` で確認オーバーレイを表示。`skip_close_confirmation_for_processes_named` でホワイトリスト登録可能。

```lua
{ key = 'w', mods = 'CMD', action = act.CloseCurrentPane { confirm = true } },
```

### 4.7 Pane の移動

`PaneSelect` のモードを利用:
- `mode = 'MoveToNewTab'` — 選択した Pane を新しい Tab に移動
- `mode = 'MoveToNewWindow'` — 選択した Pane を新しい Window に移動

Pane オブジェクトのメソッド:
- `pane:move_to_new_tab()` — Pane を新しい Tab に再配置
- `pane:move_to_new_window()` — Pane を新しい Window に再配置

---

## 5. マルチプレクサアーキテクチャ

### 5.1 コアコンセプト

WezTerm のマルチプレクサは **ドメイン（Domain）** に基づく。ドメインとは Window / Tab / Pane の独立した集合である。WezTerm 起動時にネイティブ UI 用の **デフォルトローカルドメイン** が作成される。追加ドメインを設定することで、リモートや永続的なセッション管理が可能になる。

ドメインに接続すると、リモートの Window / Tab / Pane がローカルのネイティブ GUI に統合される。tmux がテキストベースのプロトコルを使用するのに対し、WezTerm はネイティブ GUI レンダリングを提供する。

### 5.2 ドメインの種類

#### Local Domain（ローカルドメイン）

起動時に自動的に作成されるネイティブ UI ドメイン。ローカルの Tab と Window を管理する。

#### Unix Domain（Unix ドメイン）

ソケットベースの接続。Windows を含む全プラットフォームでサポートされる（WSL 統合用）。tmux の永続セッションに相当する。

```lua
config.unix_domains = {
  {
    name = 'unix',
    -- オプションフィールド:
    socket_path = '/path/to/socket',          -- カスタムソケットパス
    no_serve_automatically = false,           -- サーバーの自動起動を無効化
    skip_permissions_check = false,           -- 所有権チェックをスキップ（NTFS 用）
    local_echo_threshold_ms = 10,             -- 予測エコーの閾値（v20220319 以降）
    proxy_command = { 'nc', '-U', '/path' },  -- 外部コマンド経由でルーティング
  },
}

-- 起動時に自動接続
config.default_gui_startup_args = { 'connect', 'unix' }
```

CLI で接続: `wezterm connect unix`

#### SSH Domain（SSH ドメイン）

SSH 経由のリモートマルチプレクサ接続。マルチプレクサモードではリモートホストに WezTerm のインストールが必要。

```lua
config.ssh_domains = {
  {
    name = 'my.server',
    remote_address = '192.168.1.1',
    username = 'wez',
    -- オプションフィールド:
    no_agent_auth = false,                    -- SSH エージェント認証を無効化
    connect_automatically = false,            -- 起動時に接続
    timeout = 30,                             -- 読み取りタイムアウト（秒）
    remote_wezterm_path = '/usr/local/bin/wezterm-mux-server',
    ssh_option = { identityfile = '/path/to/key' },
    multiplexing = 'WezTerm',                -- 'WezTerm'（デフォルト）または 'None'
    assume_shell = 'Unknown',                -- 'Unknown' または 'Posix'（multiplexing='None' 時）
    default_prog = { '/bin/bash' },           -- multiplexing='None' 時のプログラム
    local_echo_threshold_ms = 10,             -- 予測エコー（v20220319 以降）
    overlay_lag_indicator = false,            -- 遅延オーバーレイの表示
  },
}
```

**自動追加（v20230408 以降）:** `~/.ssh/config` から SSH ドメインが自動的に追加される:
- `SSH:hostname` — プレーン SSH 接続（マルチプレクシングなし）
- `SSHMUX:hostname` — マルチプレクス SSH 接続

接続: `wezterm connect my.server` または `wezterm connect SSHMUX:my.server`

#### TLS Domain（TLS ドメイン）

SSH ブートストラップによる鍵交換を伴う暗号化 TCP 接続。TLS 経由の永続リモートセッションを提供する。

**クライアント側:**

```lua
config.tls_clients = {
  {
    name = 'server.name',
    remote_address = 'server.hostname:8080',
    bootstrap_via_ssh = 'server.hostname',
  },
}
```

**サーバー側:**

```lua
config.tls_servers = {
  {
    bind_address = 'server.hostname:8080',
  },
}
```

TLS ドメインはキャッシュされた証明書を使用して自動的に再接続する。

### 5.3 ドメインのアタッチ / デタッチ

| アクション | 説明 |
|-----------|------|
| `AttachDomain 'domain_name'` | ドメインに接続し、その Window / Tab / Pane をローカル GUI に統合 |
| `DetachDomain 'CurrentPaneDomain'` | 現在の Pane のドメインを切断 |
| `DetachDomain { DomainName = 'devhost' }` | 名前指定でドメインを切断 |

```lua
config.keys = {
  { key = 'U', mods = 'CTRL|SHIFT', action = act.AttachDomain 'devhost' },
  { key = 'D', mods = 'CTRL|SHIFT', action = act.DetachDomain 'CurrentPaneDomain' },
}
```

**デタッチの動作:** ドメインの Window / Tab / Pane をローカル GUI から削除するが、リモートの Pane は閉じない。再アタッチ時に復元される。

### 5.4 `default_mux_server_domain`

Mux サーバーのデフォルトドメインを制御する。全ての新しい Tab / Pane を特定のドメインにスポーンさせたい場合に有用。

---

## 6. キーバインド

### 6.1 デフォルトキーバインド一覧

#### コピー & ペースト

| バインド | アクション |
|---------|----------|
| `Super+C` / `Ctrl+Shift+C` | クリップボードにコピー |
| `Super+V` / `Ctrl+Shift+V` | クリップボードからペースト |
| `Ctrl+Insert` | プライマリセレクションにコピー |
| `Shift+Insert` | プライマリセレクションからペースト |

#### Window 操作

| バインド | アクション |
|---------|----------|
| `Super+N` / `Ctrl+Shift+N` | 新規ウィンドウ |
| `Super+M` | ウィンドウを隠す |
| `Super+H` | アプリケーションを隠す（macOS） |
| `Alt+Enter` | フルスクリーン切り替え |

#### フォントサイズ

| バインド | アクション |
|---------|----------|
| `Super+-` / `Ctrl+-` | フォント縮小 |
| `Super+=` / `Ctrl+=` | フォント拡大 |
| `Super+0` / `Ctrl+0` | フォントサイズリセット |

#### Tab 操作

| バインド | アクション |
|---------|----------|
| `Super+T` / `Ctrl+Shift+T` | 新規 Tab（現在の Pane ドメイン） |
| `Super+Shift+T` | 新規 Tab（デフォルトドメイン） |
| `Super+W` / `Ctrl+Shift+W` | Tab を閉じる |
| `Super+1`..`9` / `Ctrl+Shift+1`..`9` | 番号で Tab 切り替え（9 = 最後） |
| `Super+Shift+[` / `Ctrl+Shift+Tab` / `Ctrl+PageUp` | 前の Tab |
| `Super+Shift+]` / `Ctrl+Tab` / `Ctrl+PageDown` | 次の Tab |
| `Ctrl+Shift+PageUp` | Tab を左に移動 |
| `Ctrl+Shift+PageDown` | Tab を右に移動 |

#### Pane 操作

| バインド | アクション |
|---------|----------|
| `Ctrl+Shift+Alt+"` | 上下分割 |
| `Ctrl+Shift+Alt+%` | 左右分割 |
| `Ctrl+Shift+矢印キー` | Pane ナビゲーション |
| `Ctrl+Shift+Alt+矢印キー` | Pane リサイズ |
| `Ctrl+Shift+Z` | Pane ズーム切り替え |

#### スクロール

| バインド | アクション |
|---------|----------|
| `Shift+PageUp` | ページ上スクロール |
| `Shift+PageDown` | ページ下スクロール |

#### 検索・選択

| バインド | アクション |
|---------|----------|
| `Super+F` / `Ctrl+Shift+F` | 検索を開く |
| `Ctrl+Shift+X` | コピーモードを有効化 |
| `Ctrl+Shift+Space` | クイックセレクト |
| `Ctrl+Shift+U` | 文字セレクタ |

#### その他

| バインド | アクション |
|---------|----------|
| `Super+R` / `Ctrl+Shift+R` | 設定のリロード |
| `Super+K` / `Ctrl+Shift+K` | スクロールバックをクリア |
| `Ctrl+Shift+L` | デバッグオーバーレイ表示 |
| `Ctrl+Shift+P` | コマンドパレット |

### 6.2 カスタムキーバインド設定

```lua
local wezterm = require 'wezterm'
local act = wezterm.action
local config = wezterm.config_builder()

config.keys = {
  { key = 'x', mods = 'CTRL', action = act.SomeAction },
}
```

**修飾キー:** `SUPER` / `CMD` / `WIN`, `CTRL`, `SHIFT`, `ALT` / `OPT` / `META`, `LEADER`

複数の修飾キーはパイプで結合: `'CTRL|SHIFT|ALT'`

**デフォルトの無効化:**

```lua
config.disable_default_key_bindings = true  -- 全デフォルトを無効化

-- 個別に無効化:
{ key = 'm', mods = 'CMD', action = act.DisableDefaultAssignment },
```

**デフォルト一覧の確認:** `wezterm show-keys --lua`

### 6.3 Leader キー

tmux のプレフィックスキーに相当するモーダル修飾キー。仮想的な `LEADER` 修飾子をアクティブにする:

```lua
config.leader = { key = 'a', mods = 'CTRL', timeout_milliseconds = 1000 }

config.keys = {
  { key = '|', mods = 'LEADER|SHIFT',
    action = act.SplitHorizontal { domain = 'CurrentPaneDomain' } },
  { key = '-', mods = 'LEADER',
    action = act.SplitVertical { domain = 'CurrentPaneDomain' } },
}
```

Leader キー押下後、`LEADER` を含むバインドのみが認識される。タイムアウトまたは任意のキー押下で自動的に無効になる。

### 6.4 Key Table

名前付きキーボード設定セット。`ActivateKeyTable` で有効化する:

```lua
config.keys = {
  { key = 'r', mods = 'LEADER',
    action = act.ActivateKeyTable {
      name = 'resize_pane',
      one_shot = false,              -- Escape まで有効
      timeout_milliseconds = 3000,   -- タイムアウトで自動終了
      replace_current = true,        -- 現在のテーブルをポップ
    },
  },
  { key = 'a', mods = 'LEADER',
    action = act.ActivateKeyTable {
      name = 'activate_pane',
      timeout_milliseconds = 1000,
    },
  },
}

config.key_tables = {
  resize_pane = {
    { key = 'LeftArrow',  action = act.AdjustPaneSize { 'Left', 1 } },
    { key = 'RightArrow', action = act.AdjustPaneSize { 'Right', 1 } },
    { key = 'UpArrow',    action = act.AdjustPaneSize { 'Up', 1 } },
    { key = 'DownArrow',  action = act.AdjustPaneSize { 'Down', 1 } },
    { key = 'h', action = act.AdjustPaneSize { 'Left', 1 } },
    { key = 'j', action = act.AdjustPaneSize { 'Down', 1 } },
    { key = 'k', action = act.AdjustPaneSize { 'Up', 1 } },
    { key = 'l', action = act.AdjustPaneSize { 'Right', 1 } },
    { key = 'Escape', action = 'PopKeyTable' },
  },
  activate_pane = {
    { key = 'LeftArrow',  action = act.ActivatePaneDirection 'Left' },
    { key = 'RightArrow', action = act.ActivatePaneDirection 'Right' },
    { key = 'UpArrow',    action = act.ActivatePaneDirection 'Up' },
    { key = 'DownArrow',  action = act.ActivatePaneDirection 'Down' },
    { key = 'h', action = act.ActivatePaneDirection 'Left' },
    { key = 'j', action = act.ActivatePaneDirection 'Down' },
    { key = 'k', action = act.ActivatePaneDirection 'Up' },
    { key = 'l', action = act.ActivatePaneDirection 'Right' },
  },
}
```

スタック管理: `PopKeyTable`, `ClearKeyTableStack`。設定リロード時にスタックは自動クリアされる。

---

## 7. Lua API リファレンス

### 7.1 全 KeyAssignment アクション一覧

#### Pane 操作

| アクション | 説明 |
|-----------|------|
| `SplitPane { direction, size, command, top_level }` | 柔軟な Pane 分割 |
| `SplitHorizontal { SpawnCommand }` | 左右分割 |
| `SplitVertical { SpawnCommand }` | 上下分割 |
| `ActivatePaneDirection 'Left'\|'Right'\|'Up'\|'Down'\|'Next'\|'Prev'` | 方向指定で Pane 切り替え |
| `ActivatePaneByIndex(n)` | インデックス指定で Pane 切り替え |
| `PaneSelect { mode, alphabet, show_pane_ids }` | Pane 選択オーバーレイ |
| `AdjustPaneSize { direction, amount }` | Pane リサイズ |
| `RotatePanes 'Clockwise'\|'CounterClockwise'` | Pane 回転 |
| `TogglePaneZoomState` | ズーム切り替え |
| `SetPaneZoomState(bool)` | 明示的なズーム設定 |
| `CloseCurrentPane { confirm }` | Pane を閉じる |

#### Tab 操作

| アクション | 説明 |
|-----------|------|
| `SpawnTab 'CurrentPaneDomain'\|'DefaultDomain'\|{ DomainName = '...' }` | Tab 作成 |
| `SpawnCommandInNewTab { SpawnCommand }` | コマンド指定で Tab 作成 |
| `ActivateTab(n)` | インデックス指定で Tab 切り替え |
| `ActivateTabRelative(offset)` | 相対 Tab 切り替え（ラップあり） |
| `ActivateTabRelativeNoWrap(offset)` | 相対 Tab 切り替え（ラップなし） |
| `ActivateLastTab` | 直前の Tab に切り替え |
| `MoveTab(n)` | Tab を絶対位置に移動 |
| `MoveTabRelative(offset)` | Tab を相対位置に移動 |
| `CloseCurrentTab { confirm }` | Tab を閉じる |

#### Window 操作

| アクション | 説明 |
|-----------|------|
| `SpawnWindow` | 新規ウィンドウ |
| `SpawnCommandInNewWindow { SpawnCommand }` | コマンド指定で新規ウィンドウ |
| `ActivateWindow(n)` | ウィンドウ切り替え |
| `ActivateWindowRelative(delta)` | ウィンドウ巡回（ラップあり） |
| `ActivateWindowRelativeNoWrap(delta)` | ウィンドウ巡回（ラップなし） |
| `ToggleFullScreen` | フルスクリーン切り替え |
| `ToggleAlwaysOnTop` | 最前面固定 |
| `ToggleAlwaysOnBottom` | 最背面固定 |
| `SetWindowLevel` | ウィンドウレベル設定 |
| `StartWindowDrag` | ドラッグ開始 |
| `Hide` / `Show` | 表示/非表示 |
| `HideApplication` | アプリ非表示 |
| `QuitApplication` | アプリ終了 |

#### Workspace 操作

| アクション | 説明 |
|-----------|------|
| `SwitchToWorkspace { name, spawn }` | Workspace 切り替え（なければ作成） |
| `SwitchWorkspaceRelative(offset)` | 相対 Workspace 切り替え |

#### Domain 操作

| アクション | 説明 |
|-----------|------|
| `AttachDomain 'domain_name'` | ドメインに接続 |
| `DetachDomain 'CurrentPaneDomain'\|{ DomainName = '...' }` | ドメインを切断 |

#### 選択・コピー

| アクション | 説明 |
|-----------|------|
| `ActivateCopyMode` | コピーモード有効化 |
| `QuickSelect` | クイックセレクト |
| `QuickSelectArgs { patterns, alphabet, action, label, scope_lines }` | カスタムクイックセレクト |
| `Copy` | コピー |
| `Paste` | ペースト |
| `PasteFrom 'Clipboard'\|'PrimarySelection'` | 指定元からペースト |
| `ClearSelection` | 選択解除 |

#### UI・インタラクション

| アクション | 説明 |
|-----------|------|
| `ActivateCommandPalette` | コマンドパレット |
| `ActivateKeyTable { name, one_shot, timeout_milliseconds, replace_current }` | Key Table 有効化 |
| `PopKeyTable` | Key Table をポップ |
| `ClearKeyTableStack` | Key Table スタックをクリア |
| `CharSelect` | 文字選択 |
| `ShowLauncher` | ランチャー表示 |
| `ShowLauncherArgs { flags, title }` | ランチャー表示（引数付き） |
| `ShowTabNavigator` | Tab ナビゲーター |
| `ShowDebugOverlay` | デバッグオーバーレイ |
| `Search { ... }` | 検索 |
| `InputSelector { title, choices, action, fuzzy }` | 入力セレクタ |
| `PromptInputLine { description, action, prompt, initial_value }` | 入力プロンプト |

#### スクロール

| アクション | 説明 |
|-----------|------|
| `ScrollByLine(n)` | n 行スクロール |
| `ScrollByPage(n)` | n ページスクロール |
| `ScrollByCurrentEventWheelDelta` | ホイールイベントでスクロール |
| `ScrollToTop` | 先頭にスクロール |
| `ScrollToBottom` | 末尾にスクロール |
| `ScrollToPrompt(n)` | プロンプトにスクロール |

#### その他

| アクション | 説明 |
|-----------|------|
| `SendKey { key, mods }` | キー送信 |
| `SendString 'text'` | テキスト送信 |
| `OpenLinkAtMouseCursor` | マウスカーソル位置のリンクを開く |
| `IncreaseFontSize` / `DecreaseFontSize` / `ResetFontSize` | フォントサイズ操作 |
| `ResetTerminal` | ターミナルリセット |
| `ClearScrollback` | スクロールバッククリア |
| `ReloadConfiguration` | 設定リロード |
| `EmitEvent 'event_name'` | イベント発火 |
| `Multiple { action1, action2, ... }` | 複数アクション実行 |
| `Nop` | 何もしない |
| `DisableDefaultAssignment` | デフォルトバインドの無効化 |

### 7.2 SpawnCommand オブジェクト

Split / Spawn 系のアクションで使用される共通構造:

| フィールド | 型 | 説明 |
|-----------|------|------|
| `args` | table | コマンドと引数の配列（例: `{ 'top' }`） |
| `cwd` | string | 作業ディレクトリ |
| `set_environment_variables` | table | 環境変数のキーバリューペア |
| `domain` | string or table | `'CurrentPaneDomain'`, `'DefaultDomain'`, `{ DomainName = 'name' }` |
| `label` | string | ラベル（launch_menu で使用） |
| `position` | table | GUI ウィンドウ配置: `{ x, y, origin }`（v20230320 以降） |

---

## 8. Pane オブジェクトとメソッド

### 8.1 Pane オブジェクト（イベントコールバックで利用可能）

| メソッド | 説明 |
|---------|------|
| `pane:activate()` | Pane をアクティブにする |
| `pane:pane_id()` | 一意の Pane 識別子を取得 |
| `pane:get_title()` | Pane タイトルを取得 |
| `pane:get_current_working_dir()` | 現在の作業ディレクトリを取得 |
| `pane:get_cursor_position()` | カーソル座標を取得 |
| `pane:get_dimensions()` | Pane サイズ情報を取得 |
| `pane:get_domain_name()` | 関連ドメイン名を取得 |
| `pane:get_foreground_process_info()` | 実行中プロセスの詳細を取得 |
| `pane:get_foreground_process_name()` | フォアグラウンドプロセス名を取得 |
| `pane:get_lines_as_escapes(nlines)` | エスケープシーケンス付きでターミナル内容を取得 |
| `pane:get_lines_as_text(nlines)` | 表示行のプレーンテキストを取得 |
| `pane:get_logical_lines_as_text(nlines)` | 論理行単位でテキストを取得 |
| `pane:get_metadata()` | Pane メタデータを取得 |
| `pane:get_semantic_zone_at(x, y)` | 指定位置のセマンティックゾーンを取得 |
| `pane:get_semantic_zones()` | 全セマンティックゾーンを取得 |
| `pane:get_text_from_region(start_x, start_y, end_x, end_y)` | 領域からテキストを抽出 |
| `pane:get_text_from_semantic_zone(zone)` | セマンティックゾーン内のテキストを取得 |
| `pane:get_tty_name()` | TTY デバイス名を取得 |
| `pane:get_user_vars()` | ユーザー定義変数を取得 |
| `pane:has_unseen_output()` | 未読出力の有無を確認 |
| `pane:inject_output(text)` | Pane にコンテンツを注入 |
| `pane:is_alt_screen_active()` | 代替スクリーンの状態を確認 |
| `pane:move_to_new_tab()` | Pane を新しい Tab に移動 |
| `pane:move_to_new_window()` | Pane を新しい Window に移動 |
| `pane:mux_pane()` | 基盤のマルチプレクサ Pane にアクセス |
| `pane:paste(text)` | クリップボード内容をペースト |
| `pane:send_paste(text)` | ペーストイベントを送信 |
| `pane:send_text(text)` | テキスト入力を送信 |
| `pane:split { ... }` | Pane を分割（新しい Pane を返す） |
| `pane:tab()` | 親 Tab を取得 |
| `pane:window()` | 親 Window を取得 |

### 8.2 PaneInformation オブジェクト（軽量スナップショット）

同期イベントコールバック（Tab / Window タイトルのフォーマット等）でパフォーマンスが重要な場面で使用される。

#### 事前計算済みフィールド

| フィールド | 型 | 説明 |
|-----------|------|------|
| `pane_id` | number | 一意の Pane 識別子 |
| `pane_index` | number | レイアウト内の位置 |
| `is_active` | boolean | Tab 内でアクティブかどうか |
| `is_zoomed` | boolean | ズーム状態かどうか |
| `left` | number | 左端のセル x 座標 |
| `top` | number | 上端のセル y 座標 |
| `width` | number | 幅（セル単位） |
| `height` | number | 高さ（セル単位） |
| `pixel_width` | number | 幅（ピクセル単位） |
| `pixel_height` | number | 高さ（ピクセル単位） |
| `title` | string | Pane タイトル |
| `user_vars` | table | ユーザー変数 |

#### アクセス時に計算されるフィールド

| フィールド | 型 | 説明 |
|-----------|------|------|
| `foreground_process_name` | string | 実行ファイルパス |
| `current_working_dir` | string | 現在の作業ディレクトリ |
| `has_unseen_output` | boolean | 未読出力の有無 |
| `domain_name` | string | 関連ドメイン名 |
| `tty_name` | string | TTY デバイス名 |

---

## 9. 高度な機能

### 9.1 Workspace

Workspace は MuxWindow をグループ化するラベルで、tmux のセッションに類似する。全ての MuxWindow は必ずひとつの Workspace に属する。GUI は一度にひとつのアクティブな Workspace のみ表示する。

#### Workspace の切り替え

```lua
-- 名前指定で切り替え（なければ作成）
{ key = 'd', mods = 'ALT', action = act.SwitchToWorkspace { name = 'default' } },

-- 初期プログラム付きで切り替え
{ key = 'm', mods = 'ALT', action = act.SwitchToWorkspace {
    name = 'monitoring',
    spawn = { args = { 'top' } },
  },
},

-- ランダム名の Workspace を作成
{ key = 'w', mods = 'ALT', action = act.SwitchToWorkspace },

-- 相対ナビゲーション（辞書順）
{ key = 'n', mods = 'CTRL', action = act.SwitchWorkspaceRelative(1) },
{ key = 'p', mods = 'CTRL', action = act.SwitchWorkspaceRelative(-1) },
```

#### 対話的な Workspace 作成

```lua
{ key = 'N', mods = 'CTRL|SHIFT',
  action = act.PromptInputLine {
    description = wezterm.format {
      { Attribute = { Intensity = 'Bold' } },
      { Foreground = { AnsiColor = 'Fuchsia' } },
      { Text = 'Enter name for new workspace' },
    },
    action = wezterm.action_callback(function(window, pane, line)
      if line then
        window:perform_action(act.SwitchToWorkspace { name = line }, pane)
      end
    end),
  },
}
```

#### Workspace ピッカー（InputSelector）

```lua
{ key = 's', mods = 'ALT',
  action = wezterm.action_callback(function(window, pane)
    local workspaces = wezterm.mux.get_workspace_names()
    local choices = {}
    for _, name in ipairs(workspaces) do
      table.insert(choices, { label = name })
    end
    window:perform_action(act.InputSelector {
      title = 'Select Workspace',
      choices = choices,
      fuzzy = true,
      action = wezterm.action_callback(function(inner_window, inner_pane, id, label)
        if label then
          inner_window:perform_action(
            act.SwitchToWorkspace { name = label }, inner_pane)
        end
      end),
    }, pane)
  end),
}
```

#### Workspace ランチャー

```lua
{ key = '9', mods = 'ALT',
  action = act.ShowLauncherArgs { flags = 'FUZZY|WORKSPACES' } },
```

#### `wezterm.mux` Workspace 関数

| 関数 | 説明 |
|------|------|
| `wezterm.mux.get_active_workspace()` | 現在の Workspace 名を取得 |
| `wezterm.mux.get_workspace_names()` | 全 Workspace 名を一覧 |
| `wezterm.mux.set_active_workspace(name)` | Workspace を切り替え |
| `wezterm.mux.rename_workspace(old_name, new_name)` | Workspace をリネーム |

#### スタートアップイベントでの Workspace 初期化

```lua
wezterm.on('gui-startup', function(cmd)
  -- GUI 初期化時に発火
  local tab, pane, window = wezterm.mux.spawn_window { workspace = 'main' }
  wezterm.mux.spawn_window { workspace = 'monitoring', args = { 'htop' } }
  wezterm.mux.set_active_workspace 'main'
end)

wezterm.on('mux-startup', function()
  -- マルチプレクサ起動時に発火（mux サーバーでも）
  local tab, pane, window = wezterm.mux.spawn_window {}
  pane:split { direction = 'Top', size = 0.333 }
end)
```

### 9.2 Copy Mode

`ActivateCopyMode`（デフォルト: `Ctrl+Shift+X`）で有効化。Vim ライクなキーボード駆動のテキスト選択を提供する。

#### 選択モード

| モード | キー | 説明 |
|-------|------|------|
| Cell | `v` | 文字単位の選択（デフォルト） |
| Line | `Shift+V` | 行単位の選択 |
| Block | `Ctrl+V` | 矩形選択（v20220624 以降） |

#### ナビゲーションキー

| 操作 | キー |
|------|------|
| 上下左右移動 | 矢印キー / `h`/`j`/`k`/`l` |
| 単語前方 | `w`, `Alt+Right`, `Tab`, `Alt+F` |
| 単語後方 | `b`, `Alt+Left`, `Shift+Tab`, `Alt+B` |
| 行頭 | `0` / `Home` |
| 行末 | `$` / `End` |
| スクロールバック先頭 | `g` |
| スクロールバック末尾 | `Shift+G` |
| ページ上 | `PageUp` / `Ctrl+B` |
| ページ下 | `PageDown` / `Ctrl+F` |
| 半ページ上 | `Ctrl+U` |
| 半ページ下 | `Ctrl+D` |
| 選択をコピーして終了 | `y` |
| コピーせず終了 | `Escape`, `Ctrl+C`, `Ctrl+G`, `q` |

`copy_mode` Key Table はカスタマイズ可能。デフォルト確認: `wezterm show-keys --lua --key-table copy_mode`

### 9.3 Quick Select

`QuickSelect`（デフォルト: `Ctrl+Shift+Space`）で有効化。パターンにマッチするテキストをハイライトし、1キーのラベルで選択できる。

```lua
-- 基本的なクイックセレクト
{ key = 'Space', mods = 'SHIFT|CTRL', action = act.QuickSelect },

-- カスタムパターンとアクション
{ key = 'u', mods = 'SHIFT|CTRL',
  action = act.QuickSelectArgs {
    patterns = { 'https?://\\S+' },   -- URL のみマッチ
    alphabet = 'asdfqwerzxcv',         -- カスタムラベルアルファベット
    label = 'open url',                -- オーバーレイラベルテキスト
    scope_lines = 500,                 -- 検索行数（デフォルト: 1000）
    action = wezterm.action_callback(function(window, pane)
      local url = window:get_selection_text_for_pane(pane)
      wezterm.open_with(url)
    end),
  },
},
```

関連設定:
- `quick_select_alphabet` — ラベルのデフォルトアルファベット
- `quick_select_patterns` — 追加の正規表現パターン
- `disable_default_quick_select_patterns` — ビルトインパターンの無効化

### 9.4 ShowLauncher / ShowLauncherArgs

```lua
-- ファジー Workspace ピッカー
{ key = '9', mods = 'ALT',
  action = act.ShowLauncherArgs { flags = 'FUZZY|WORKSPACES' } },

-- 全ドメイン
{ key = '0', mods = 'ALT',
  action = act.ShowLauncherArgs { flags = 'FUZZY|DOMAINS' } },

-- Tab スイッチャー
{ key = 'Tab', mods = 'CTRL',
  action = act.ShowLauncherArgs { flags = 'FUZZY|TABS' } },

-- 統合ランチャー
{ key = 'l', mods = 'ALT',
  action = act.ShowLauncherArgs {
    flags = 'FUZZY|TABS|LAUNCH_MENU_ITEMS|DOMAINS|WORKSPACES|COMMANDS',
    title = 'Launcher',
  },
},
```

**利用可能なフラグ:** `FUZZY`, `TABS`, `LAUNCH_MENU_ITEMS`, `DOMAINS`, `KEY_ASSIGNMENTS`, `WORKSPACES`, `COMMANDS`

---

## 10. リモートマルチプレクシング

### 10.1 Unix Domain — ローカル永続セッション

tmux セッションに最も近い機能。バックグラウンドで mux サーバーが動作し、GUI がそこに接続する。GUI を閉じてもサーバーは動作を続け、再接続で復元される。

```lua
config.unix_domains = {
  { name = 'unix' },
}
config.default_gui_startup_args = { 'connect', 'unix' }
```

接続: `wezterm connect unix`

手動でサーバー起動: `wezterm-mux-server --daemonize`

### 10.2 SSH Domain — リモート永続セッション

```lua
config.ssh_domains = {
  {
    name = 'devbox',
    remote_address = 'devbox.example.com',
    username = 'dev',
    multiplexing = 'WezTerm',  -- リモートに wezterm が必要（デフォルト）
  },
}
```

接続: `wezterm connect devbox` または `wezterm connect SSHMUX:devbox`

WezTerm 未インストールのサーバー向け:

```lua
{
  name = 'plain-ssh',
  remote_address = 'server.example.com',
  multiplexing = 'None',       -- プレーン SSH、永続性なし
  assume_shell = 'Posix',
}
```

### 10.3 TLS Domain — 暗号化 TCP 経由の永続セッション

```lua
-- クライアント
config.tls_clients = {
  {
    name = 'prod-server',
    remote_address = 'prod.example.com:8080',
    bootstrap_via_ssh = 'prod.example.com',
  },
}

-- サーバー（リモートホスト上）
config.tls_servers = {
  { bind_address = 'prod.example.com:8080' },
}
```

TLS はキャッシュされた証明書で自動再接続する。SSH ブートストラップが初回の鍵交換を処理する。

### 10.4 レイテンシマスキング

高遅延接続向けに予測ローカルエコーを有効化:

```lua
config.unix_domains = {
  {
    name = 'unix',
    local_echo_threshold_ms = 10,   -- RTT > 10ms で有効化
  },
}
```

`ssh_domains` でも `local_echo_threshold_ms` で利用可能。

### 10.5 WSL 統合

**WSL 1（AF_UNIX 互換）:**

WSL 内:

```lua
config.unix_domains = {
  {
    name = 'wsl',
    socket_path = '/mnt/c/Users/USERNAME/.local/share/wezterm/sock',
    skip_permissions_check = true,
  },
}
```

Windows ホスト:

```lua
config.unix_domains = {
  {
    name = 'wsl',
    serve_command = { 'wsl', 'wezterm-mux-server', '--daemonize' },
  },
}
config.default_gui_startup_args = { 'connect', 'wsl' }
```

### 10.6 CLI コマンド

```bash
wezterm connect <domain_name>              # ドメインに GUI 接続
wezterm cli spawn --domain-name <name>     # 既存 GUI インスタンスにスポーン
wezterm cli split-pane --help              # CLI で Pane 分割
wezterm-mux-server --daemonize             # mux サーバーをデーモン起動
```

---

## 11. 設定例

### 11.1 tmux ライクな完全セットアップ

```lua
local wezterm = require 'wezterm'
local act = wezterm.action
local config = wezterm.config_builder()

-- Leader キー（tmux プレフィックスに相当）
config.leader = { key = 'a', mods = 'CTRL', timeout_milliseconds = 1000 }

config.keys = {
  -- Pane 分割
  { key = '|', mods = 'LEADER|SHIFT',
    action = act.SplitHorizontal { domain = 'CurrentPaneDomain' } },
  { key = '-', mods = 'LEADER',
    action = act.SplitVertical { domain = 'CurrentPaneDomain' } },

  -- Pane ナビゲーション
  { key = 'h', mods = 'LEADER', action = act.ActivatePaneDirection 'Left' },
  { key = 'j', mods = 'LEADER', action = act.ActivatePaneDirection 'Down' },
  { key = 'k', mods = 'LEADER', action = act.ActivatePaneDirection 'Up' },
  { key = 'l', mods = 'LEADER', action = act.ActivatePaneDirection 'Right' },

  -- Pane リサイズ
  { key = 'H', mods = 'LEADER|SHIFT',
    action = act.AdjustPaneSize { 'Left', 5 } },
  { key = 'J', mods = 'LEADER|SHIFT',
    action = act.AdjustPaneSize { 'Down', 5 } },
  { key = 'K', mods = 'LEADER|SHIFT',
    action = act.AdjustPaneSize { 'Up', 5 } },
  { key = 'L', mods = 'LEADER|SHIFT',
    action = act.AdjustPaneSize { 'Right', 5 } },

  -- Pane ズーム
  { key = 'z', mods = 'LEADER', action = act.TogglePaneZoomState },

  -- Pane クローズ
  { key = 'x', mods = 'LEADER',
    action = act.CloseCurrentPane { confirm = true } },

  -- Pane 選択
  { key = 'q', mods = 'LEADER', action = act.PaneSelect },
  { key = 'Q', mods = 'LEADER|SHIFT',
    action = act.PaneSelect { mode = 'SwapWithActive' } },

  -- Pane 回転
  { key = 'Space', mods = 'LEADER', action = act.RotatePanes 'Clockwise' },

  -- Tab 操作
  { key = 'c', mods = 'LEADER',
    action = act.SpawnTab 'CurrentPaneDomain' },
  { key = 'p', mods = 'LEADER', action = act.ActivateTabRelative(-1) },
  { key = 'n', mods = 'LEADER', action = act.ActivateTabRelative(1) },
  { key = 'o', mods = 'LEADER', action = act.ActivateLastTab },

  -- Tab リネーム
  { key = ',', mods = 'LEADER',
    action = act.PromptInputLine {
      description = 'Enter new tab name',
      action = wezterm.action_callback(function(window, pane, line)
        if line then window:active_tab():set_title(line) end
      end),
    },
  },

  -- Workspace 管理
  { key = 's', mods = 'LEADER',
    action = act.ShowLauncherArgs { flags = 'FUZZY|WORKSPACES' } },
  { key = 'w', mods = 'LEADER',
    action = act.PromptInputLine {
      description = 'Enter workspace name',
      action = wezterm.action_callback(function(window, pane, line)
        if line then
          window:perform_action(
            act.SwitchToWorkspace { name = line }, pane)
        end
      end),
    },
  },

  -- Domain アタッチ / デタッチ
  { key = 'd', mods = 'LEADER',
    action = act.DetachDomain 'CurrentPaneDomain' },
  { key = 'a', mods = 'LEADER|SHIFT',
    action = act.ShowLauncherArgs { flags = 'FUZZY|DOMAINS' } },

  -- リサイズモード Key Table
  { key = 'r', mods = 'LEADER',
    action = act.ActivateKeyTable {
      name = 'resize_pane', one_shot = false },
  },
}

-- Tab 番号バインド
for i = 1, 9 do
  table.insert(config.keys, {
    key = tostring(i), mods = 'LEADER', action = act.ActivateTab(i - 1),
  })
end

-- Key Tables
config.key_tables = {
  resize_pane = {
    { key = 'h', action = act.AdjustPaneSize { 'Left', 1 } },
    { key = 'j', action = act.AdjustPaneSize { 'Down', 1 } },
    { key = 'k', action = act.AdjustPaneSize { 'Up', 1 } },
    { key = 'l', action = act.AdjustPaneSize { 'Right', 1 } },
    { key = 'Escape', action = 'PopKeyTable' },
    { key = 'Enter', action = 'PopKeyTable' },
  },
}

return config
```

### 11.2 Workspace 初期化レイアウト

```lua
wezterm.on('gui-startup', function(cmd)
  -- メインコーディング Workspace
  local tab, pane, window = wezterm.mux.spawn_window {
    workspace = 'coding',
    cwd = '/home/user/projects',
  }
  pane:split { direction = 'Bottom', size = 0.3, cwd = '/home/user/projects' }

  -- モニタリング Workspace
  local mon_tab, mon_pane, mon_window = wezterm.mux.spawn_window {
    workspace = 'monitoring',
    args = { 'htop' },
  }
  mon_pane:split {
    direction = 'Right',
    args = { 'tail', '-f', '/var/log/syslog' },
  }

  -- サーバー Workspace（SSH）
  wezterm.mux.spawn_window {
    workspace = 'servers',
    domain = { DomainName = 'SSHMUX:production' },
  }

  -- コーディング Workspace でスタート
  wezterm.mux.set_active_workspace 'coding'
end)
```

### 11.3 永続セッション（Unix Domain）

```lua
config.unix_domains = {
  { name = 'unix' },
}
config.default_gui_startup_args = { 'connect', 'unix' }
```

### 11.4 リモート開発セットアップ

```lua
config.ssh_domains = {
  {
    name = 'devbox',
    remote_address = 'devbox.internal:22',
    username = 'developer',
    multiplexing = 'WezTerm',
    remote_wezterm_path = '/home/developer/.local/bin/wezterm-mux-server',
    local_echo_threshold_ms = 10,
  },
}

config.tls_clients = {
  {
    name = 'prod-bastion',
    remote_address = 'bastion.prod.example.com:8080',
    bootstrap_via_ssh = 'bastion.prod.example.com',
  },
}
```

---

## 参考リンク

- [WezTerm 公式ドキュメント](https://wezfurlong.org/wezterm/)
- [デフォルトキーバインド](https://wezterm.org/config/default-keys.html)
- [キーバインド設定](https://wezterm.org/config/keys.html)
- [Key Tables](https://wezterm.org/config/key-tables.html)
- [マルチプレクシング](https://wezterm.org/multiplexing.html)
- [プログラムの起動](https://wezterm.org/config/launch.html)
- [Workspace / セッション](https://wezterm.org/recipes/workspaces.html)
- [Copy Mode](https://wezterm.org/copymode.html)
- [KeyAssignment インデックス](https://wezterm.org/config/lua/keyassignment/index.html)
- [SpawnCommand オブジェクト](https://wezterm.org/config/lua/SpawnCommand.html)
- [PaneInformation オブジェクト](https://wezterm.org/config/lua/PaneInformation.html)
- [Pane オブジェクトメソッド](https://wezterm.org/config/lua/pane/index.html)
- [MuxTab オブジェクト](https://wezterm.org/config/lua/MuxTab/index.html)
- [MuxWindow オブジェクト](https://wezterm.org/config/lua/mux-window/index.html)
- [wezterm.mux モジュール](https://wezterm.org/config/lua/wezterm.mux/index.html)
- [SshDomain オブジェクト](https://wezterm.org/config/lua/SshDomain.html)
