# MyFax UI spec (screens, interaction, i18n)

Status: DRAFT v1, written against repo commit `be763c5`. Spec only: nothing in this file has been compiled or
rendered. Every pixel value is a starting value to be confirmed by the first screenshot (see section 12).
Audience: Haiku implementers (views, controller, i18n). Follow literally; anything marked `OPEN` is a question for
the orchestrator (section 13).

Sources read: `BRIEF.md`, `src/app/msg.rs`, `src/app/state.rs`, `src/model.rs`, `src/config.rs`,
`src/ui/runtime/mod.rs`, `src/i18n/mod.rs`, and the AcmeUI widget/style/theme/layout crates (pinned checkout).

---------------------------------------------------------------------------------------------------

## 0. Ground rules

### 0.1 What a view may use
* Views are pure functions `&AppState -> WidgetNode<Msg>`; they read only `AppState` (and `Local::now()` once per
  frame where stated, passed down as a parameter so helpers stay pure and testable).
* Widgets: `row()`, `column()`, `stack()`, `card()`, `label_builder()`, `button()`, `separator()`, `text_input()` only.
* Colours: `ColorToken` only (`bg_*`, `bg(ColorToken::X)`, `text_muted()`, `text_color(ColorToken::X)`,
  `border(1.0, ColorToken::Border)`). NEVER `ColorToken::Direct`, never `LabelBuilder::color(ThemeColor)`, no hex/rgb.
* Text: only through `t(state.lang, "area.key")` / `tf(state.lang, "area.key", &[("name", &value)])` with a string
  literal key. Keys that depend on an enum are chosen by a `match` whose arms each hold a literal
  (`match folder { Folder::All => t(l, "inbox.folder_all"), ... }`). Never `format!` a key. Glyphs, digits,
  dates and file sizes are not translated (see 0.4).
* Spacing on a 4 px grid: 4, 8, 12, 16, 20, 24, 32. Radii from tokens (`rounded_sm` 4 inputs/chips-in-lists,
  `rounded_md` 6 buttons/rows, `rounded_lg` 8 cards/dialog, `rounded_full` badges/pills).

### 0.2 Layout engine facts the sketches rely on (verified in `acme-layout`)
* Flexbox (taffy). `Styled::flex_1()` = `flex_grow 1`; `flex_shrink` default 0. Fixed panes use `.w(N)`;
  the flexible pane uses `.flex_1()` + `.min_w(N)`.
* `overflow_scroll()` on a keyed `column()`/`row()` makes a scroll region and (taffy) lets it shrink below content,
  so scroll regions are `flex_1()` children of a column with a fixed-height siblings. Always give scroll regions
  a key (table in 10).
* `stack()` = block display and EVERY child is absolutely positioned at the stack's top-left, auto-sized to its
  content. So each layer of a stack that should cover the window MUST set `.w_full().h_full()`.
  There is no align/justify API: centring is done with flex spacers (`column().flex_1()` empty) on both sides.
* No wrap layout: chip groups are explicit rows of a known count. No ellipsis: long text is shortened in the view
  with `truncate_units` (0.4). Labels default to no-wrap; use `label_builder(..).wrap(LabelWrap::Word)` (prose) or
  `LabelWrap::Character` (paths, file names) when a value may be long, and only inside a width-constrained parent.
* `LabelBuilder` is `Styled`: `.font_size(f)`, `.font_weight(FontWeight::SemiBold)`, `.text_muted()`,
  `.text_color(ColorToken::X)` all work before `.build()`.
* Button: `button(key, label).variant(ButtonVariant::{Primary,Secondary,Ghost,Danger}).size(ButtonSize::{XS 24,Small 30,Medium 36,Large 42}).disabled(b).leading_icon(glyph).on_click(msg)` -> `WidgetNode`. `on_click` is last.
* TextInput: `text_input(field.key()).placeholder(..).value(state.field(field)).invalid(b).validation(msg).disabled(b).build()`.
  Key MUST be `Field::key()`. No message attached (typing arrives as `UiEvent::TextChanged`). Single-line only:
  the note fields are single-line too.
* Clickable row = `row().key(..).on_click(msg)`; nested `Button`s inside win the hit-test over the row.

### 0.3 Runtime assumptions (see OPEN list in 13)
* A1: runtime paints hover/pressed overlay (GhostHover/GhostPressed) on every clickable container; views do nothing for hover.
* A2: a container with no `on_click` is NOT a hit-test target (clicks pass through to what is beneath).
* A3: `Button` width is intrinsic unless `.w()`/`.full_width(true)`; `column()` stretches children horizontally,
  `row()` stretches vertically. Wrap an auto-width button in a `row()`. Where width matters, give an explicit `.w()`.
* A4: a container whose key starts with `modal:` is a hit-test target that swallows pointer events (no message).
* A5: the focus ring (`ColorToken::Ring`, 2 px) is drawn by the runtime for the focused TextInput/Button.

### 0.4 Shared view kit (one file, `src/ui/views/kit.rs`, owned by the first views worker; all pages import it)
Metrics (consts): `NAV_W 152`, `FILTER_W 224`, `DETAIL_W 352`, `LIST_MIN_W 280`, `STATUS_H 28`, `BANNER_H 40`,
`PAGE_PAD 24`, `PANE_PAD 12`, `ROW_H 56`, `CHART_H 160`, `PREVIEW_MAX_H 420`, `SENDER_MAX_UNITS 24`.
Helpers (all return `WidgetNode<Msg>` unless noted):
```
title(text)            label 20 SemiBold, Foreground
section(text)          label 15 SemiBold, Foreground
body(text)             label 14, Foreground
caption(text)          label 12, text_muted
caption_wrap(text)     caption + LabelWrap::Word
badge(text, Tone)      row px8 py2 rounded_full bg(<Tone>Soft) [label 12 text_color(<Tone>)]
chip(text, Tone)       same as badge but rounded_sm (tags)
spacer()               column().flex_1().build()
hgap(h)/vgap(w)        column().h(h) / row().w(w) empty fixed spacer
vdivider()             column().w(1).h_full().bg(Border)
hdivider()             separator()
field(label, input)    column gap 4 [caption(label), input]
page_header(title, subtitle: Option<String>, actions: Vec<WidgetNode>)
                       row h56 px24 py12 [column[title, subtitle?], spacer, row gap8 actions] then hdivider
empty_state(glyph, title, body, Option<action>)   centred block (spacers), see 3.8
```
Pure functions (unit-test them): `truncate_units(s, max)` (unit = 1 for ASCII, 2 for any other char; append `…`
when cut, never cut inside a char; `max` in units), `truncate_middle(s, max)` (for paths, keeps head and tail),
`fmt_dt(DateTime<Local>) -> "YYYY-MM-DD HH:mm"`, `fmt_size(u64)` (`< 1024` -> `"{n} B"`, `< 1 MiB` -> `"{:.0} KB"`,
else `"{:.1} MB"`), `fmt_percent(zoom) -> "{}%"` rounded, `tone_tokens(Tone) -> (bg ColorToken, fg ColorToken)`.

`Tone` -> (bg, fg): Neutral (Muted, MutedForeground); Info (InfoSoft, Info); Success (SuccessSoft, Success);
Warning (WarningSoft, Warning); Danger (DangerSoft, Danger); Primary (Primary, PrimaryForeground).

Glyphs (one `mod glyph` of `&str` consts in kit; candidates, confirm they render in section 12 step 2):
`STAR "★"`, `STAR_OFF "☆"`, `DOT "●"`, `RING "○"`, `RADIO_ON "◉"`, `CHECK_ON "☑"`, `CHECK_OFF "☐"`,
`CHEVRON_RIGHT "▸"`, `CHEVRON_DOWN "▾"`, `PREV "‹"`, `NEXT "›"`, `CLOSE "×"`, `OK "✓"`, `MINUS "−"`, `PLUS "+"`,
`NAV_INBOX "✉"`, `NAV_CONTACTS "☺"`, `NAV_SEND "➤"`, `NAV_STATS "▥"`, `NAV_LOG "≡"`, `NAV_SETTINGS "⚙"`,
`WARN "!"`, `INFO "i"`.

### 0.5 Notation in the tree sketches
`Kind#key [style props]` with children indented. `{expr}` = computed value; `if` = conditional child.
Styles: `p12` = padding 12, `px/py`, `gap8`, `w224`, `h_full`, `flex_1`, `scroll` = `overflow_scroll`, `bg:Surface` = `bg(ColorToken::Surface)`.
`T:` = i18n key. Every `Button` states variant/size/msg. Spacing numbers are px.

---------------------------------------------------------------------------------------------------

## 1. Visual direction

Restrained Windows-11-style desktop utility. No hero, no gradients, no decorative colour. Hierarchy by size and
weight plus a single accent (Primary) used only for: active nav indicator, primary buttons, unread dot/bold, the
selected chip, bars in charts. Structure by surfaces: window `Background`; nav rail, filter pane, detail pane
and cards `Surface`; list pane `Background`; 1 px `Border` dividers between panes; cards `CardVariant::Outlined`.
Dialog and toast use `CardVariant::Elevated` (views have no `Theme`, so no explicit shadow calls); the dialog background is `SurfaceOverlay`.

Typography scale (the only sizes allowed): title 20 SemiBold; metric 28 SemiBold (stats cards only);
section 15 SemiBold; body 14; caption 12 (muted). Buttons use their own size. One font family (theme default).

Colour use (semantic, both themes by construction; contrast pairs allowed):
Foreground on Background; SurfaceForeground on Surface; PrimaryForeground on Primary; AccentForeground on Accent;
`X` on `XSoft` for status; MutedForeground on Background/Surface. FORBIDDEN: MutedForeground on Accent or Muted
(inside a selected row all text is `AccentForeground`; hierarchy there is size/weight only).

State table:
| Element | default | hover | pressed | selected/active | focus | disabled | invalid |
|---|---|---|---|---|---|---|---|
| Button Secondary/Ghost/Primary/Danger | variant style | runtime (A1) | runtime | Primary variant used as "on" | Ring (A5) | `.disabled(true)` | n/a |
| Clickable row (fax, contact, tree node, nav, printer) | Surface or Background | GhostHover overlay (A1) | GhostPressed | `bg:Accent` + 3 px left bar `Primary` + text `AccentForeground` | Ring | n/a | n/a |
| TextInput | theme | theme | n/a | n/a | Ring + `Ring` border | `.disabled(true)` | `.invalid(true).validation(msg)` |
| Unread fax row | sender SemiBold + `●` Primary | | | | | | |
| Toast / banner | by kind tone | click dismiss | | | | | |

Left-bar trick: every selectable row is `row [column#bar w3 (bg Primary if active else bg of the row), content]`
so text does not shift when selected.

---------------------------------------------------------------------------------------------------

## 2. Shell (`views::root` composes this; owner: shell worker)

Window 1024x680 min, 1280x800 default. Root is a `stack` of up to four layers (later = on top):

```
Stack#root [w_full h_full]
  L0  Column#shell [w_full h_full bg:Background]
        Row#body [flex_1]
          Column#nav [w152 h_full bg:Surface]            (2.1)
          vdivider()
          Column#pagearea [flex_1 h_full]
            if state.new_fax_banner > 0 -> banner (2.3)  <- in flow, NOT an overlay (never hides page header)
            page(state)  [flex_1]                        (3..8)  one of the six pages by state.route
        hdivider()
        Row#statusbar [h28 px12 gap12 bg:Surface]        (2.2)
  L1  if state.toast.is_some():  Column#toast_layer [w_full h_full]   (2.4)   pass-through (A2)
  L2  if state.confirm.is_some():
        Column#modal:scrim [w_full h_full bg:Foreground opacity 0.35]       (2.5) swallows input (A4)
  L3  if state.confirm.is_some():  Column#modal:dialog_layer [w_full h_full]  (2.5) centred dialog
```
Z-order (bottom to top): shell < toast < scrim < dialog. A toast raised while a dialog is open stays below the
scrim (intended; controller should not raise toasts while `confirm` is open except errors, which are then hidden
until it closes: acceptable). Layers are present only when active (no empty placeholder layers).
Scrim and dialog are siblings so `opacity` never dims the dialog.

### 2.1 Nav rail (152 wide, `bg:Surface`, padding 8, gap 4)
```
Column#nav [w152 h_full bg:Surface p8 gap4]
  Column#brand [px8 py8 gap0]
    label "MyFax" 20 SemiBold             T:shell.app_name
    caption                                T:shell.app_tagline
  hgap(8)
  for route in Route::ALL:  navitem(route)
  spacer()
  Column#watchmini [px8 py8 gap4]          watcher status (short)
    Row gap6 [label "●"/"○" tone-coloured, caption {short status}]
    if Running(n): caption {n}
```
`navitem(route)`: `row#nav.<route_snake> [h36 px8 gap8 rounded_md on_click Msg::Navigate(route)]` where
`route_snake` in {inbox, contacts, send, stats, log, settings}:
`[column#bar w3 rounded_full (Primary if state.route==route else transparent=Surface bg)]`
`[label glyph 14 w20]` `[label name 14 flex_1 (SemiBold if active)]` `[if Inbox && counts.unread>0: badge(unread, Primary)]`.
Active: `bg:Accent`, text `AccentForeground`. Unread badge text: `n` or `99+` when n > 99 (not translated).
Labels: `shell.nav_inbox|contacts|send|stats|log|settings`; glyphs `NAV_*`.
Watcher short status: Running -> glyph `DOT` Success + `shell.watch_running_short`; Stopped -> `RING` muted +
`shell.watch_stopped_short`; Error -> `DOT` Danger + `shell.watch_error_short`.

### 2.2 Status bar (28 high)
```
Row#statusbar [h28 px12 gap12 bg:Surface]
  [watcher: glyph + text]     Running(n): T:shell.status_watching{count} (Success text)
                              Stopped && settings.watch_folders.is_empty(): T:shell.status_no_folder (Warning text)
                              Stopped: T:shell.status_stopped (muted)
                              Error(e): T:shell.status_error{error=truncate_units(e,60)} (Danger text)
  caption "·"
  caption  last_scan: Some(t) -> T:shell.last_scan{time}  (HH:mm:ss if today else YYYY-MM-DD HH:mm:ss)
                      None    -> T:shell.no_scan
  caption "·"
  caption#statusbar.archive  T:shell.archive_path{path=truncate_middle(settings.archive_root, 56)}  flex_1
  Button#btn.scan_now Ghost XS  T:shell.scan_now  -> Msg::ScanNow
```
Text of status items is caption size 12.

### 2.3 New-fax banner (in flow, height 40)
Shown when `new_fax_banner > 0`; sits above the page inside `pagearea`.
```
Row#banner [h40 px16 gap12 bg:InfoSoft]
  label DOT 14 text:Info
  label T:shell.banner_new_faxes{count} 14 SemiBold text:Info flex_1
  if state.route != Route::Inbox:
     Button#btn.banner_view Secondary XS  T:shell.banner_view -> Msg::Navigate(Route::Inbox)
  Button#btn.banner_dismiss Ghost XS  label "×"  -> Msg::DismissBanner
```
Deliberate deviation from "overlay": an in-flow banner cannot hide a page header or toolbar, and still disappears
the moment it is dismissed. Controller rule: `Navigate(Inbox)` and `DismissBanner` both set `new_fax_banner = 0`.

### 2.4 Toast (layer L1)
Bottom-centre, 8 px above the status bar. Never blocks input (A2).
```
Column#toast_layer [w_full h_full]
  spacer()                                   flex_1
  Row [w_full]                               
    spacer()
    Row#toast [Card Elevated, max_w520, px16 py12 gap12, bg:{tone soft}, border 1 {tone}, on_click Msg::DismissToast]
       label glyph (OK "✓" Success / WARN "!" Danger / INFO "i" Info), 14 SemiBold, text:{tone}
       label {toast.text} 14 wrap Word, text:Foreground, flex_1
       label "×" 14 text_muted
    spacer()
  vgap(36)                                   STATUS_H 28 + 8
```
Kind -> tone: Info->Info, Success->Success, Error->Danger. The whole toast is the dismiss target. Auto-dismiss is
a controller timer (Info/Success 4 s, Error 8 s); the view has no timers.

### 2.5 Confirm dialog (layers L2 + L3)
```
Column#modal:scrim [w_full h_full bg:Foreground opacity 0.35]           (swallows input, A4)
Column#modal:dialog_layer [w_full h_full]                              centring by spacers
  spacer()
  Row [w_full]
    spacer()
    Card#modal:dialog [Elevated, w420, p20, gap16, bg:SurfaceOverlay]  (A4: swallows clicks inside)
      label {confirm.title} 15 SemiBold
      label {confirm.body}  14 wrap Word text:Foreground
      Row#dialog.buttons [gap8]
        spacer()
        Button#btn.confirm_no  Secondary Medium  T:common.cancel -> Msg::ConfirmNo
        Button#btn.confirm_yes Danger    Medium  {label by action} -> Msg::ConfirmYes
    spacer()
  spacer()
```
Confirm button label by `confirm.action`: `DeletePermanently` -> `shell.confirm_delete`; `EmptyTrash` -> `shell.confirm_empty`;
`DeleteContact` -> `shell.confirm_delete`; `ClearLog` -> `shell.confirm_clear`.
Behaviour: Esc (`UiEvent::Cancel`) = `ConfirmNo`. While `confirm.is_some()` the controller ignores every Msg except
`ConfirmYes`, `ConfirmNo`, `DismissToast` (keyboard focus can still reach widgets under the scrim). Enter does not
confirm (destructive action needs a deliberate click or Tab+Space). Cancel is the first button in tree order.

---------------------------------------------------------------------------------------------------

## 3. Inbox page (Route::Inbox)

```
Row#inbox [flex_1 h_full]
  Column#filters  [w224 h_full bg:Surface]          3.1
  vdivider()
  Column#list     [flex_1 min_w280 h_full bg:Background]   3.2
  vdivider()
  Column#detail   [w352 h_full bg:Surface]          3.3
```
At 1024 the list is 1024-152-1-224-1-352-1 = 293 px wide (>= LIST_MIN_W). Vertical budget at 1024x680 (page area
651 px; 611 with banner): list toolbar 124 + pager 41 + bottom blocks 121 = 286 fixed, leaves 365 px (6.5 rows)
for `scroll.list`; 325 with the banner.

### 3.1 Filter column
```
Column#filters [w224 h_full bg:Surface]
  Row [h40 px12 gap8]
    section T:inbox.filters  flex_1
    Button#btn.clear_filters Ghost XS  T:inbox.clear_filters -> Msg::ClearFilters   disabled = !filters_active
  hdivider()
  Column#scroll.filters [flex_1 scroll p12 gap16]
    (A) Folders
      caption T:inbox.section_folders
      for f in [All, Unread, Starred, Trash]:
        row#folder.<all|unread|starred|trash> [h32 px8 gap8 rounded_md on_click Msg::SelectFolder(f)]
           bar w3, label name 14 flex_1, label {count} 12   (count from state.counts)
           active when state.query.folder == f
    (B) Date
      caption T:inbox.section_date
      chips: 4 rows, 2 per row, gap8, each Button w102 Small:
        [AnyTime|Today] [Yesterday|Last7Days] [Last30Days|ThisMonth] [ThisYear]
        Primary variant when state.date_preset == preset else Secondary
        key btn.preset.<any|today|yesterday|7d|30d|month|year> -> Msg::SetDatePreset(preset)
        label by match (T:inbox.preset_any|today|yesterday|7d|30d|month|year)
      TextInput Field::DateFrom  placeholder T:inbox.date_from_ph   (invalid=false)
      TextInput Field::DateTo    placeholder T:inbox.date_to_ph
      Button#btn.apply_range Secondary Small T:inbox.date_apply -> Msg::ApplyCustomRange
      (when date_preset == Custom the two inputs get no special state; the range is shown via the date tree highlight)
    (C) Date tree
      caption T:inbox.section_tree
      if state.date_tree.is_empty(): caption T:inbox.tree_empty
      for y in date_tree (already newest first):
        row#tree.y.<year> [h32 gap0 rounded_md]
           Button#btn.toggle_year.<year> Ghost XS label (CHEVRON_DOWN if y.expanded else CHEVRON_RIGHT) w28 -> Msg::ToggleYear(year)
           row#tree.ysel.<year> [flex_1 h_full px8 gap8 on_click Msg::SelectYear(year)]
              label "{year}" 14 SemiBold, spacer, label "{count}" 12
        if y.expanded: for m in y.months (newest first):
           row#tree.m.<year>-<mm> [h28 pl28 pr8 gap8 rounded_md on_click Msg::SelectMonth{year,month}]
              label "{mm}" (two-digit month; zh: "{m} 月" is NOT used, keep numeric), spacer, label "{count}" 12
           if state.expanded_month == Some((year, month)): for d in m.days (ascending):
              row#tree.d.<yyyy-mm-dd> [h28 pl48 pr8 gap8 rounded_md on_click Msg::SelectDay(date)]
                 label "{dd}" two-digit, spacer, label "{count}" 12
      active node = derived from state.query.date_from/date_to:
        year   active if from==Jan 1 && to==Dec 31 of that year
        month  active if from==first day && to==last day of that month
        day    active if from==to==that day
    (D) Tags
      caption T:inbox.section_tags
      if state.all_tags.is_empty(): caption T:inbox.tags_empty
      for (tag,count) in all_tags.take(30):
        Button#btn.tag.<tag> Small (Primary if query.tag == Some(tag) else Ghost)
           label "#{truncate_units(tag,16)}  {count}"
           -> Msg::SetTagFilter( if active { None } else { Some(tag) } )
```
`filters_active` = `!query.text.is_empty() || query.date_from.is_some() || query.date_to.is_some() || query.tag.is_some()`
(folder is not a filter for this purpose). Tree month label: two digits (`03`) with the year row above giving context.
Rows with `bar w3` follow the left-bar trick (1). Folder-row count for Trash uses `counts.trash`.

### 3.2 List column
```
Column#list [flex_1 min_w280 h_full bg:Background]
  Column#toolbar [p12 gap8]
    TextInput Field::Search  placeholder T:inbox.search_ph   (w_full)
    Row#sortrow [gap8]
      for s in [NewestFirst, OldestFirst, SenderAsc]:
        Button#btn.sort.<newest|oldest|sender> Small  Primary if query.sort==s else Secondary
           T:inbox.sort_newest|sort_oldest|sort_sender  -> Msg::SetSort(s)
    Row#metarow [h24 gap8]
      caption T:inbox.result_count{count=results.total}  flex_1
      if folder==Trash && counts.trash>0:
         Button#btn.empty_trash Danger XS T:inbox.empty_trash_btn -> Msg::AskEmptyTrash
      else Button#btn.mark_all_read Ghost XS T:inbox.mark_all_read -> Msg::MarkAllRead   disabled = counts.unread==0
  hdivider()
  Column#scroll.list [flex_1 scroll]
    if results.items.is_empty(): empty state (3.8)
    else for f in results.items: faxrow(f)
  hdivider()
  Row#pager [h40 px12 gap8]
    Button#btn.prev_page Secondary Small  "‹ " + T:common.prev  disabled = query.page==0 -> Msg::PrevPage
    caption T:common.page_x_of_y{page=query.page+1,pages=state.page_count()}  flex_1 (centred via spacers either side)
    Button#btn.next_page Secondary Small  T:common.next + " ›"  disabled = query.page+1 >= page_count -> Msg::NextPage
  hdivider()
  Column#bottom [p12 gap8 bg:Surface]
    Row#importrow [gap8]
      TextInput Field::ImportPath placeholder T:inbox.import_ph  flex_1
      Button#btn.browse.import_path Secondary Small label T:common.browse_short w36 -> Msg::Browse(Field::ImportPath)
      Button#btn.import Primary Small T:common.import -> Msg::ImportFromField
    caption_wrap T:inbox.import_hint
    Row#exportrow [gap8]
      TextInput Field::ExportFolder placeholder T:inbox.export_ph flex_1
      Button#btn.browse.export_folder Secondary Small T:common.browse_short w36 -> Msg::Browse(Field::ExportFolder)
      Button#btn.export Secondary Small T:inbox.export_btn -> Msg::ExportFiltered  disabled = results.total==0
```
`faxrow(f)` (height 56, key `row.fax.<id>`):
```
row#row.fax.<id> [h56 rounded_md on_click Msg::SelectFax(f.id)]  bg:Accent if state.selected.id==f.id else none
  column#bar w3 (Primary if selected)
  Column [flex_1 px12 py8 gap2]
    Row [gap8]                                         line 1
      label (if !f.is_read {DOT} else "") w12 text:Primary
      label (if f.is_starred {STAR} else "") w14 text:Warning   (STAR keeps the Warning tone: the only non-brand colour in a row)
      label {truncate_units(sender, SENDER_MAX_UNITS)} 14 flex_1   SemiBold if !is_read else Normal
            sender = f.display_sender() or T:inbox.unknown_sender
      label T:common.pages_short{pages} 12
    Row [gap8]                                         line 2
      caption {fmt_dt(f.received_at)}
      for tag in f.tags.take(2): chip("#"+truncate_units(tag,10), Info)
      if f.tags.len()>2: chip(T:inbox.tag_more{count=len-2}, Neutral)
```
In a selected row use `AccentForeground` for all labels (also the dot/star keep their own tone). Fax rows in Trash look
identical (the folder header already says Trash). Row stripes/dividers: a hairline `separator()` between rows is not
needed; row hover + 56 px rhythm is enough.

### 3.3 Detail pane (352 wide)
```
Column#detail [w352 h_full bg:Surface]
  if state.selected.is_none(): empty state (3.8, detail variant)
  else:
    Row#detailhead [h40 px12 gap8]
      section T:detail.title flex_1
      Button#btn.close_detail Ghost XS "×" -> Msg::CloseDetail
    hdivider()
    Column#scroll.detail [flex_1 scroll p12 gap12]
      (1) info card
      (2) actions
      (3) edit card
      (4) preview
```
(1) Info card: `card().variant(Outlined).padding(12).gap(8)`
```
label {sender display or T:inbox.unknown_sender} 20 SemiBold  wrap Word
if f.sender_name non-empty && f.sender_number is Some: caption {number}
if f.deleted_at is Some(t): row px8 py4 rounded_sm bg:WarningSoft [label T:detail.in_trash{time=fmt_dt(t)} 12 text:Warning]
kv(T:detail.field_received, fmt_dt(received_at))
kv(T:detail.field_number,   sender_number or "—")
kv(T:detail.field_pages,    "{pages}")
kv(T:detail.field_size,     fmt_size(file_size))
kv(T:detail.field_original, original_name)       value wrap Character
kv(T:detail.field_path,     file_path.display())  value wrap Character
```
`kv(label, value)` = `row gap8 [caption(label) w84, label value 14 flex_1 wrap]`.
(2) Actions: `column gap8` of rows (each `row gap8`, buttons Small):
```
Row A  Button#btn.toggle_read  Secondary  (is_read ? T:detail.mark_unread : T:detail.mark_read) -> ToggleRead(id)
       Button#btn.toggle_star  Secondary  (is_starred ? STAR+" "+T:detail.unstar : STAR_OFF+" "+T:detail.star) -> ToggleStar(id)
Row B  Button#btn.open   Primary    T:detail.open   -> OpenExternal(id)
       Button#btn.print  Secondary  T:detail.print  -> Print(id)
       Button#btn.reveal Secondary  T:detail.reveal -> RevealInFolder(id)
Row C  if sender_number is Some:
         if a contact exists with fax_number == normalize_number(sender_number): caption T:detail.in_contacts
         else Button#btn.add_contact Secondary T:detail.add_contact -> AddSenderToContacts(id)
Row D  if deleted_at.is_none():  Button#btn.trash Danger T:detail.trash -> MoveToTrash(id)
       else: Button#btn.restore Secondary T:detail.restore -> Restore(id)
             Button#btn.delete_forever Danger T:detail.delete_forever -> AskDeletePermanently(id)
```
(3) Edit card: `card().variant(Outlined).padding(12).gap(8)`
```
section T:detail.section_edit
field(T:detail.label_sender, TextInput Field::FaxSenderName)
field(T:detail.label_tags,   TextInput Field::FaxTags)  + caption T:detail.tags_hint
field(T:detail.label_note,   TextInput Field::FaxNote)
Row [spacer, Button#btn.save_edits Primary Small T:detail.save_edits -> Msg::SaveFaxEdits(id)]
```
The controller fills the three `fields` on `SelectFax` (sender_name or "", tags joined with `", "`, note).
(4) Preview: `column gap8`
```
section T:detail.section_preview
match state.preview.status:
  Empty (selected is Some): Card Muted h120 [centered caption T:detail.preview_loading]
  Unavailable(text):  Card Muted p12 gap8 [ label WARN w16 text:Warning, caption_wrap {text}, caption_wrap T:detail.preview_unavailable_hint,
                       Button#btn.preview_open Primary Small T:detail.open -> OpenExternal(id) ]
  Ready:
    Row#previewbar [gap4]
      Button#btn.preview_prev Ghost XS "‹" disabled=page==0 -> PreviewPrevPage
      caption T:common.page_x_of_y{page=page+1,pages=page_count}
      Button#btn.preview_next Ghost XS "›" disabled=page+1>=page_count -> PreviewNextPage
      spacer()
      Button#btn.zoom_out   Ghost XS "−" disabled=zoom<=0.5 -> ZoomOut
      caption T:detail.zoom_percent{percent}
      Button#btn.zoom_in    Ghost XS "+" disabled=zoom>=3.0 -> ZoomIn
      Button#btn.zoom_reset Ghost XS T:detail.zoom_fit -> ZoomReset
    Column#scroll.preview [w_full h{min(h_px, PREVIEW_MAX_H)} scroll bg:Muted border 1 Border rounded_sm]
      Column#img:preview [w{w_px} h{h_px}]            (image slot; runtime paints the page into it)
```
Slot size (pure fn `preview_slot(preview, inner_w=328) -> (w_px, h_px)`): `w_px = round(328 * zoom)`; if
`image_width>0 && image_height>0`: `h_px = round(w_px * image_height / image_width)` else `h_px = round(w_px * 1.414)`
(A4). Because the slot ratio equals the image ratio the "contain" fit has no letter-boxing. At zoom 1.0 the page
exactly fits the pane width; above 1.0 the viewport scrolls both ways; below 1.0 the page is smaller than the
viewport. Known trade-off: wheel over the viewport scrolls the viewport, not the detail pane; the viewport is capped
at 420 px so the rest of the pane stays reachable from the cards above and below.

### 3.4 Empty states (`empty_state(glyph, title, body, action)`)
Centred: `Column [flex_1] [spacer, row[spacer, column gap8 (glyph 28 text_muted, title 15 SemiBold, caption_wrap body (max_w 240), action?), spacer], spacer]`.
| Where | When | glyph | title | body | action |
|---|---|---|---|---|---|
| list | folder==Trash && no items | `NAV_INBOX` | `inbox.empty_trash` | — | — |
| list | `filters_active` or folder != All | `NAV_INBOX` | `inbox.empty_title` | `inbox.empty_filtered` | Secondary Small `inbox.clear_filters` -> ClearFilters |
| list | no filters && `settings.watch_folders.is_empty()` | `NAV_INBOX` | `inbox.empty_title` | `inbox.empty_no_folder` | Primary Small `inbox.go_settings` -> Navigate(Settings) |
| list | otherwise | `NAV_INBOX` | `inbox.empty_title` | `inbox.empty_waiting` | — |
| detail | selected None | `NAV_INBOX` | `detail.empty_title` | `detail.empty_hint` | — |

Evaluate rows in the order listed (first match wins).

### 3.5 Inbox rules
* Search is live: every `TextChanged` of `field.search` re-queries from page 0; `Submit` re-queries too.
* Page changes, filter changes and sort changes reset `scroll.list` to the top; `SelectFax` resets `scroll.detail` and `scroll.preview`
  (app loop calls `UiRuntime::reset_scroll`).
* A selected fax stays selected when it falls off the current page/filter (the detail pane keeps showing it); the row simply is not highlighted.
* Moving to trash / delete clears `selected` when it is that fax (controller).
* Drag and drop of files/folders onto the window = `ImportFromField`-equivalent handled by the platform layer; the only UI is `inbox.import_hint`.

---------------------------------------------------------------------------------------------------

## 4. Contacts page (Route::Contacts)

```
Column#page.contacts [flex_1]
  page_header(T:contacts.title, Some(T:contacts.count{count=contacts.len()}),
              [Button#btn.new_contact Primary Small T:contacts.new -> Msg::NewContact])
  Row#contactsbody [flex_1]
    Column#contactlist [flex_1 min_w360 h_full bg:Background]
      Column#contactsearch [p12]  TextInput Field::ContactSearch placeholder T:contacts.search_ph
      hdivider()
      Column#scroll.contacts [flex_1 scroll]
        if contacts.is_empty(): empty_state(NAV_CONTACTS, contacts.empty_title, contacts.empty_hint)
        else if filtered.is_empty(): empty_state(NAV_CONTACTS, contacts.no_match, —)
        else for c in filtered: contactrow(c)
      hdivider()
      Column#csvblock [p12 gap8 bg:Surface]
        caption T:contacts.csv_section
        Row gap8 [ TextInput Field::CsvPath placeholder T:contacts.csv_ph flex_1,
                   Button#btn.browse.csv_path Secondary Small T:common.browse -> Browse(Field::CsvPath) ]
        Row gap8 [ Button#btn.csv_import Secondary Small T:contacts.csv_import -> ImportContactsCsv,
                   Button#btn.csv_export Secondary Small T:contacts.csv_export -> ExportContactsCsv ]
    vdivider()
    Column#contacteditor [w380 h_full bg:Surface]           4.2
```
`filtered` = contacts where the lower-cased `ContactSearch` text (split on whitespace, every word) is a substring of
`name + company + fax_number + phone + email`. The view filters; the controller does NOT (it keeps the full list).
`contactrow(c)` (key `row.contact.<id>`, h56, on_click `EditContact(c.id)`, selected when `editing_contact == Some(c.id)`):
```
row [h56 rounded_md on_click EditContact(id)]
  bar w3
  Column [flex_1 px12 py8 gap2]
    label {truncate_units(name,28)} 14 SemiBold
    caption {truncate_units(join(" · ", [company, fax_number] non-empty), 40)}
  Button#btn.contact_send.<id> Ghost XS T:contacts.send_to -> Msg::UseContactForSend(id)    (mr12, inside the row)
```
### 4.2 Editor
```
Column#contacteditor [w380 h_full bg:Surface]
  match state.editing_contact:
    None => empty_state(NAV_CONTACTS, contacts.editor_empty, —, Primary Small contacts.new -> NewContact)
    Some(id) =>
      Row [h40 px16] section (id==0 ? T:contacts.editor_new_title : T:contacts.editor_edit_title)
      hdivider()
      Column#scroll.contact_editor [flex_1 scroll p16 gap12]
        field(T:contacts.field_name,    TextInput Field::ContactName)
        field(T:contacts.field_company, TextInput Field::ContactCompany)
        field(T:contacts.field_fax,     TextInput Field::ContactFax)  + caption T:contacts.fax_hint
        field(T:contacts.field_phone,   TextInput Field::ContactPhone)
        field(T:contacts.field_email,   TextInput Field::ContactEmail)
        field(T:contacts.field_note,    TextInput Field::ContactNote)
      hdivider()
      Row#editorbuttons [p12 gap8]
        Button#btn.contact_save   Primary   Small T:common.save   -> SaveContact
        Button#btn.contact_cancel Secondary Small T:common.cancel -> CancelContactEdit
        spacer()
        if id != 0: Button#btn.contact_delete Danger Small T:common.delete -> AskDeleteContact(id)
```
Validation is the controller's (toasts `app.contact_invalid_name|fax`); the view shows no inline error because
AppState carries none. The controller fills the six `Contact*` fields on `NewContact` (empty) / `EditContact(id)`.

---------------------------------------------------------------------------------------------------

## 5. Send page (Route::Send)

The document is handed to the printer's fax driver; the driver shows its own dialog for the number. MyFax cannot
dial, cannot confirm delivery.
```
Column#page.send [flex_1]
  page_header(T:send.title, None, [])
  Row#sendbody [flex_1]
    Column#scroll.send_form [flex_1 min_w420 scroll p24 gap16]
      Card#send.explain [Outlined p16 gap8 bg:InfoSoft]
        section T:send.explain_title
        caption_wrap T:send.explain_body  (14px body-size label, wrap Word)
      (1) printers
        Row gap8 [ section T:send.section_printer flex_1,
                   Button#btn.refresh_printers Secondary Small T:common.refresh -> RefreshPrinters ]
        if printers.is_empty(): Card Muted p12 [caption_wrap T:send.printers_empty]
        else for name in order_printers(&printers):
          row#printer.<index> [h36 px8 gap8 rounded_md on_click SelectPrinter(name)]   selected = selected_printer == Some(name)
             bar w3, label (RADIO_ON if selected else RING) text:Primary, label {truncate_units(name,48)} 14 flex_1,
             if is_fax(name): badge(T:send.printer_fax_badge, Info)
      (2) file
        section T:send.section_file
        Row gap8 [ TextInput Field::SendFile placeholder T:send.file_ph flex_1,
                   Button#btn.browse.send_file Secondary Small T:common.browse -> Browse(Field::SendFile) ]
      (3) recipient (for the user's own records)
        section T:send.section_recipient
        TextInput Field::SendRecipient placeholder T:send.recipient_ph
        caption T:send.pick_contact
        Column#scroll.send_contacts [h132 scroll border 1 Border rounded_md]
          if contacts.is_empty(): caption padded T:send.contacts_empty
          else for c in contacts: row#sc.<id> [h36 px12 gap8 on_click UseContactForSend(c.id)]
                 label name 14 flex_1, caption fax_number
        TextInput Field::SendNote placeholder T:send.note_ph
      (4) go
        if selected_printer.is_none(): caption text:Warning T:send.need_printer
        if field(SendFile).trim().is_empty(): caption text:Warning T:send.need_file
        Row [ Button#btn.send_fax Primary Large  leading_icon NAV_SEND  T:send.button -> Msg::SendFax ]
    vdivider()
    Column#scroll.send_history [w360 scroll p16 gap8 bg:Surface]
      section T:send.history_title
      if sent.is_empty(): caption T:send.history_empty
      else for s in sent.take(100): Card Outlined p12 gap2
             label {file_name(s.file_path) truncate_units 36} 14 SemiBold
             caption {fmt_dt(s.at)}
             caption T:send.history_via{printer}
             if !recipient.is_empty(): caption T:send.history_to{recipient}
             if !note.is_empty(): caption_wrap {note}
```
`is_fax(name)` = lower-cased name contains `"fax"`. `order_printers` = stable partition: fax printers first, then others
(each group keeps input order). `SendFax` button stays enabled even when prerequisites are missing: the controller
answers with `app.send_no_printer` / `send.need_file`-style toasts (so keyboard users always get feedback); the
yellow captions above it explain why. After a successful hand-over the controller toasts `app.send_handed`.
`UseContactForSend` fills `SendRecipient` with the contact name and fax number (`"{name} {fax}"`), controller-side;
it does not navigate (we are already on this page; if triggered from the contacts page the controller also
navigates to `Route::Send`).

---------------------------------------------------------------------------------------------------

## 6. Stats page (Route::Stats)

```
Column#page.stats [flex_1]
  page_header(T:stats.title, None, [3 range buttons])
      Button#btn.range.7d   Small (Primary if stats_range==Last7Days   else Secondary) T:stats.range_7d   -> SetStatsRange(Last7Days)
      Button#btn.range.30d  ... T:stats.range_30d -> SetStatsRange(Last30Days)
      Button#btn.range.365d ... T:stats.range_365d -> SetStatsRange(Last365Days)
  Column#scroll.stats [flex_1 scroll p24 gap16]
    Row#metrics [gap12]                       four cards, each flex_1 (min_w 0)
      metric(T:stats.total_faxes, "{total_faxes}")
      metric(T:stats.total_pages, "{total_pages}")
      metric(T:stats.avg_pages,   total_faxes==0 ? "—" : "{:.1}" of total_pages/total_faxes)
      metric(T:stats.busiest_day, per_day.max_by(count) -> T:stats.busiest_value{date,count} or "—")
    Card#chartcard [Outlined p16 gap12]
      Row [ section (monthly ? T:stats.chart_monthly : T:stats.chart_daily) flex_1,
            caption T:stats.chart_peak{count=max} ]
      chart                                    6.1
      caption T:stats.chart_range{from,to}     (dates YYYY-MM-DD; for monthly: first/last month YYYY-MM)
    Card#topcard [Outlined p16 gap8]
      section T:stats.top_senders
      if top_senders.is_empty(): caption T:stats.top_empty
      else for (i,(name,count)) in top_senders.take(10):
        Row#top.<i> [h28 gap12]
          caption "{i+1}" w24
          label {truncate_units(name,28)} 14 w220
          Column [w{bar_w} h12 ... ]  bar: row [column w{round(count/max*160), min 4} h12 bg:Primary rounded_sm]  inside a row w160 h12
          label T:stats.sender_count{count} 12 w56
```
`metric(label, value)`: `Card Outlined p16 gap4 flex_1 [caption(label), label value 28 SemiBold]`.

### 6.1 Chart (no chart widget; fixed-height coloured containers)
Pure fn `chart_data(range, &stats.per_day, today) -> ChartData { buckets: Vec<Bucket{label, count, show_label}>, max: u64, monthly: bool, from: NaiveDate, to: NaiveDate }`:
* `Last7Days`: 7 buckets, dates `today-6 ..= today`, missing day = 0, label `MM-DD`, `show_label` always.
* `Last30Days`: 30 buckets, dates `today-29 ..= today`, label `DD`, `show_label` when `index % 5 == 0 || index == 29`.
* `Last365Days`: 12 monthly buckets ending with the current month (sum of `per_day` by year-month), label `MM`, `show_label` always, `monthly = true`.
* `max` = largest bucket count (min 1 to avoid divide by zero when drawing).
Sizing: bar width / gap = 7d: 40 / 12; 30d: 16 / 6; 12m: 36 / 12. Total width <= 664, fits 824 px of inner width at 1024.
```
Column#chart [gap4]
  Row#bars [gap{GAP}]   (spacer() before and after so the group is centred)
    for (i,b) in buckets:
      Column#bar.<i> [w{BAR_W} h{CHART_H + 16}]
        spacer()                                     flex_1 pushes content to the bottom
        if buckets.len() <= 12 && b.count>0: label "{count}" 12 (centred by w{BAR_W} column; text_muted)
        Column [w{BAR_W} h{bar_h} bg:Primary rounded_sm]      bar_h = if count==0 {2} else {max(2, round(count/max*CHART_H))}
                                                                count==0 uses bg:Border instead of Primary
  hdivider()
  Row#xlabels [gap{GAP}] same spacers: for b: Column [w{BAR_W}] [caption(b.label) if show_label]
if total_faxes == 0: overlay-free empty message: Row [spacer, caption T:stats.chart_empty, spacer] placed between bars and baseline
```
Bars have no click/hover behaviour (no tooltip widget): numbers are shown above bars for the 7- and 12-bucket modes and the
peak is in the card header. Zero buckets still render a 2 px `Border` stub so the time axis is continuous.
`today` = `Local::now().date_naive()` read once in the page fn.

---------------------------------------------------------------------------------------------------

## 7. Log page (Route::Log)

```
Column#page.log [flex_1]
  page_header(T:log.title, Some(T:log.count{count=logs.len()}),
              [Button#btn.clear_log Danger Small T:log.clear -> AskClearLog  disabled = logs.is_empty()])
  if logs.is_empty(): empty_state(NAV_LOG, log.empty, —)
  else:
    Column#scroll.log [flex_1 scroll px24 py8]
      for e in logs.take(500):
        Row#log.<id> [px0 py8 gap12]
          caption {YYYY-MM-DD HH:mm:ss} w152
          badge(level text, tone)  w64 (centred by the badge row inside Column w64)
          label {message} 14 wrap Word flex_1
        separator()
      if logs.len() > 500: caption T:log.truncated{count=500}
```
Level -> badge: `Info` -> text `log.level_info`, tone Success (SuccessSoft/Success); `Warn` -> `log.level_warn`, Warning;
`Error` -> `log.level_error`, Danger. (Deviation noted: the brief asked for success/warning/danger, `LogLevel` has Info, so
Info uses the success tone. One-line change in `tone_for_level` if the orchestrator prefers the Info tone.)
Entries are shown in the order given (controller supplies newest first).

---------------------------------------------------------------------------------------------------

## 8. Settings page (Route::Settings)

```
Column#page.settings [flex_1]
  page_header(T:settings.title, None, [])
  Column#scroll.settings [flex_1 scroll p24 gap16]        content column max_w 720 (each card w_full inside Column max_w720)
    Card#set.general [Outlined p16 gap12]
      section T:settings.section_general
      caption T:settings.language
      Row gap8: for l in Lang::ALL: Button#btn.lang.<code> Small (Primary if state.lang==l else Secondary)  label l.native_name()  (not translated) -> SetLanguage(l)
      caption T:settings.theme
      Row gap8: Button#btn.theme.light Small (Primary if state.theme==Light else Secondary) T:settings.theme_light -> SetTheme(ThemeChoice::Light)
                Button#btn.theme.dark  ...                                    T:settings.theme_dark  -> SetTheme(ThemeChoice::Dark)
    Card#set.receive [Outlined p16 gap12]
      section T:settings.section_receive
      caption T:settings.watch_folders
      if settings.watch_folders.is_empty(): caption T:settings.watch_empty
      for (i,p) in settings.watch_folders.iter().enumerate():
        Row#watch.<i> [h32 gap8] label {truncate_middle(p,64)} 14 flex_1,
                                 Button#btn.watch_remove.<i> Ghost XS T:common.remove -> RemoveWatchFolder(i)
      Row gap8 [ TextInput Field::WatchFolderInput placeholder T:settings.watch_ph flex_1,
                 Button#btn.browse.watch_folder Secondary Small T:common.browse -> Browse(Field::WatchFolderInput),
                 Button#btn.watch_add Primary Small T:common.add -> AddWatchFolder ]
      field(T:settings.poll_interval, TextInput Field::PollInterval)  + caption T:settings.poll_hint
      toggles (4) — see below
      field(T:settings.forward_folder, Row gap8 [TextInput Field::ForwardFolder placeholder T:settings.forward_ph flex_1,
                                                 Button#btn.browse.forward_folder Secondary Small T:common.browse -> Browse(Field::ForwardFolder)])
    Card#set.storage [Outlined p16 gap12]
      section T:settings.section_storage
      field(T:settings.archive_root, Row gap8 [TextInput Field::ArchiveRoot flex_1,
                                               Button#btn.browse.archive_root Secondary Small T:common.browse -> Browse(Field::ArchiveRoot),
                                               Button#btn.open_archive Secondary Small T:settings.open_archive -> OpenArchiveFolder])
      caption T:settings.archive_hint
      field(T:settings.trash_retention, TextInput Field::TrashRetention) + caption T:settings.trash_hint
      field(T:settings.page_size,       TextInput Field::PageSize)       + caption T:settings.page_hint
    Card#set.help [Outlined p16 gap8 bg:InfoSoft]         "Epson printer setup" (8.2)
  hdivider()
  Row#settingsfooter [h56 px24 gap12 bg:Surface]
    Button#btn.save_settings Primary Medium T:settings.save_btn -> SaveSettings
    if dirty: caption text:Warning T:settings.unsaved
    caption_wrap T:settings.apply_note  flex_1
```
Toggles: for each `(BoolSetting, current bool, key T:settings.opt_*)` in order
`DeleteSourceAfterImport (settings.delete_source_after_import, opt_delete_source)`,
`AcceptTiff (accept_tiff, opt_accept_tiff)`, `AutoPrint (auto_print, opt_auto_print)`,
`NotifyNewFax (notify_new_fax, opt_notify)`:
`Row [ Button#btn.toggle.<delete_source|accept_tiff|auto_print|notify> Ghost Small  label "{CHECK_ON|CHECK_OFF} {text}" -> ToggleSetting(b) ]`
(the wrapping `row` keeps the button auto-width; label is left-aligned by being the only content). Switches apply immediately.

Inline validation (pure fns on `state.field(..)`, used for `.invalid(..).validation(msg)`): `ArchiveRoot` non-empty (`settings.err_archive`);
`PollInterval` parses as `u64` in `2..=3600` (`settings.err_poll`); `TrashRetention` parses as `u32` in `0..=3650`
(`settings.err_trash`); `PageSize` parses as `u32` in `10..=500` (`settings.err_page`). `SaveSettings` stays enabled (the controller re-validates,
toast `app.settings_invalid`). `dirty` = any of the 7 text fields differs from its persisted value
(`archive_root.display()`, `poll_interval_secs`, `trash_retention_days`, `page_size`, `forward_copy_folder` or "", plus
`WatchFolderInput` non-empty counts as dirty because the folder was typed but not added).
Controller fills the fields from `settings` at start-up and after every successful save.

### 8.2 Epson printer setup help card (static text, `InfoSoft` card)
```
Card#set.help [Outlined p16 gap8 bg:InfoSoft]
  section T:settings.help_title
  caption_wrap T:settings.help_intro
  body-size labels (wrap Word): T:settings.help_step1, help_step2, help_step3, help_step4
  caption_wrap T:settings.help_note
```
Content (see i18n): on the printer's Web Config (or control panel) set Fax Output to Save to Network Folder (SMB)
pointing at a folder shared from this PC, choose PDF (recommended; TIFF only if "Accept TIFF" is on), then add that
same folder under Watch folders and test with Scan now.

---------------------------------------------------------------------------------------------------

## 9. Keyboard, focus and recovery

* Tree order == visual order == Tab order (runtime-defined). Do not reorder with `stack`.
* `Esc` (`UiEvent::Cancel`): confirm open -> `ConfirmNo`; else the runtime has already cleared focus; nothing else.
* `Enter` (`UiEvent::Submit{key}`) map (controller):
| Field | Effect |
|---|---|
| `field.search` | re-query now (no-op if already live) |
| `field.date_from`, `field.date_to` | `ApplyCustomRange` |
| `field.import_path` | `ImportFromField` |
| `field.watch_folder_input` | `AddWatchFolder` |
| `field.contact_name/company/fax/phone/email/note` | `SaveContact` |
| `field.fax_sender_name/tags/note` | `SaveFaxEdits(selected id)` |
| `field.export_folder`, `field.csv_path`, `field.send_*`, settings number/path fields | none (focus stays) |
* `TextChanged` stores into `AppState::fields` for every field; only `field.search` also re-queries.
* Space/Enter on a focused Button or clickable row activates it (runtime). There is no list arrow-key navigation (no Msg exists);
  suggested future Msgs: `SelectNextFax`, `SelectPrevFax`, `FocusSearch`. Not part of this spec.
* Recovery paths: every destructive action is reversible or confirmed: trash -> Restore; permanent delete, empty trash,
  contact delete, log clear -> confirm dialog with Cancel first; Contact editor -> Cancel; unsaved settings -> caption + Save;
  failed import/export/print/open -> error toast with the reason; preview failure -> Unavailable block with `Open`.
* Loading states: the store is synchronous, so lists have no skeletons. The only loading state is preview
  (`PreviewStatus::Empty` while a fax is selected -> `detail.preview_loading`).
* Error states: toasts (Error kind) for operations; Unavailable block for preview; watcher Error in status bar and nav rail; field-level
  `.invalid()` for settings numbers; there is no global error page.

---------------------------------------------------------------------------------------------------

## 10. Key registry (all widget keys must be unique in one frame; scroll offsets persist per key)

Scroll regions: `scroll.filters`, `scroll.list`, `scroll.detail`, `scroll.preview`, `scroll.contacts`,
`scroll.contact_editor`, `scroll.send_form`, `scroll.send_contacts`, `scroll.send_history`, `scroll.stats`,
`scroll.log`, `scroll.settings`. Image slot: `img:preview`. Text inputs: `Field::key()` (`field.*`). Hit-blocking: `modal:scrim`,
`modal:dialog_layer`, `modal:dialog`. Rows: `nav.<route>`, `folder.<name>`, `tree.y.<year>`, `tree.ysel.<year>`, `tree.m.<year>-<mm>`,
`tree.d.<yyyy-mm-dd>`, `row.fax.<id>`, `row.contact.<id>`, `printer.<index>`, `sc.<id>`, `watch.<i>`, `log.<id>`, `top.<i>`, `bar.<i>`.
Buttons: `btn.<action>[.<arg>]` as written in the sketches (`btn.preset.*`, `btn.sort.*`, `btn.tag.<tag>`, `btn.toggle.*`, `btn.lang.*`, `btn.range.*` ...).
The app loop calls `reset_scroll` for: `scroll.list` on page/filter/sort change; `scroll.detail` + `scroll.preview` on `SelectFax`;
every `scroll.*` of a page on `Navigate` away from it is NOT required (offsets persisting per page is intended).

Controller rules this spec depends on (collected):
1. Fill `fields` from `settings` at start-up/after save; fill fax edit fields on `SelectFax`; fill contact fields on `NewContact/EditContact`.
2. `Navigate(Inbox)` and `DismissBanner` zero `new_fax_banner`. Toast timers: Info/Success 4 s, Error 8 s.
3. While `confirm.is_some()` ignore all Msg except `ConfirmYes/No`, `DismissToast`. `UiEvent::Cancel` with confirm open = `ConfirmNo`.
4. Search is live. Contacts search is view-side (controller keeps full list). Printer ordering is view-side.
5. Log lines are written in the language active at write time using the `app.log_*` keys.
6. `ConfirmDialog.title/body` are built from `app.confirm_*` keys with `{name}`/`{count}`.

---------------------------------------------------------------------------------------------------

## 11. i18n key table

Rules: rows go to `src/i18n/catalog/<fragment>.rs` as `("key", "zh-TW", "English")`. Placeholders identical in both columns.
Taiwan wording. In Rust source a backslash is written `\\`. A leading space or glyph prefix is added by code, not in the table.
Fragment owner: `common` kit/shell worker; `shell` shell worker; `inbox`+`detail` inbox worker; `contacts`, `send`, `stats`, `log`,
`settings` their page workers; `app` controller worker.

### 11.1 common
| key | zh-TW | English |
|---|---|---|
| common.save | 儲存 | Save |
| common.cancel | 取消 | Cancel |
| common.browse | 瀏覽… | Browse... |
| common.browse_short | … | ... |
| common.delete | 刪除 | Delete |
| common.close | 關閉 | Close |
| common.apply | 套用 | Apply |
| common.add | 新增 | Add |
| common.remove | 移除 | Remove |
| common.import | 匯入 | Import |
| common.export | 匯出 | Export |
| common.refresh | 重新整理 | Refresh |
| common.open | 開啟 | Open |
| common.unknown | 未知 | Unknown |
| common.prev | 上一頁 | Previous |
| common.next | 下一頁 | Next |
| common.page_x_of_y | {page} / {pages} | {page} / {pages} |
| common.pages_short | {pages} 頁 | {pages} pg |

### 11.2 shell
| key | zh-TW | English |
|---|---|---|
| shell.app_name | MyFax | MyFax |
| shell.app_tagline | 傳真收件匣 | Fax inbox |
| shell.nav_inbox | 收件匣 | Inbox |
| shell.nav_contacts | 聯絡人 | Contacts |
| shell.nav_send | 傳送傳真 | Send fax |
| shell.nav_stats | 統計 | Statistics |
| shell.nav_log | 活動記錄 | Activity log |
| shell.nav_settings | 設定 | Settings |
| shell.watch_running_short | 監看中 | Watching |
| shell.watch_stopped_short | 已停止 | Stopped |
| shell.watch_error_short | 發生錯誤 | Error |
| shell.status_watching | 監看中：{count} 個資料夾 | Watching {count} folder(s) |
| shell.status_stopped | 資料夾監看已停止 | Folder watcher stopped |
| shell.status_no_folder | 尚未設定監看資料夾 | No watch folder configured |
| shell.status_error | 監看發生錯誤：{error} | Watcher error: {error} |
| shell.last_scan | 上次掃描：{time} | Last scan: {time} |
| shell.no_scan | 尚未掃描 | Not scanned yet |
| shell.archive_path | 封存位置：{path} | Archive: {path} |
| shell.scan_now | 立即掃描 | Scan now |
| shell.banner_new_faxes | 收到 {count} 份新傳真 | {count} new fax(es) received |
| shell.banner_view | 檢視收件匣 | View inbox |
| shell.confirm_delete | 刪除 | Delete |
| shell.confirm_empty | 清空垃圾桶 | Empty trash |
| shell.confirm_clear | 清除 | Clear |

### 11.3 inbox
| key | zh-TW | English |
|---|---|---|
| inbox.filters | 篩選 | Filters |
| inbox.clear_filters | 清除篩選 | Clear filters |
| inbox.section_folders | 傳真分類 | Folders |
| inbox.folder_all | 全部傳真 | All faxes |
| inbox.folder_unread | 未讀 | Unread |
| inbox.folder_starred | 已加星號 | Starred |
| inbox.folder_trash | 垃圾桶 | Trash |
| inbox.section_date | 日期 | Date |
| inbox.preset_any | 不限 | Any time |
| inbox.preset_today | 今天 | Today |
| inbox.preset_yesterday | 昨天 | Yesterday |
| inbox.preset_7d | 近 7 天 | 7 days |
| inbox.preset_30d | 近 30 天 | 30 days |
| inbox.preset_month | 本月 | This month |
| inbox.preset_year | 今年 | This year |
| inbox.date_from_ph | 起始日 YYYY-MM-DD | From YYYY-MM-DD |
| inbox.date_to_ph | 結束日 YYYY-MM-DD | To YYYY-MM-DD |
| inbox.date_apply | 套用日期範圍 | Apply range |
| inbox.section_tree | 依日期瀏覽 | Browse by date |
| inbox.tree_empty | 尚無傳真 | No faxes yet |
| inbox.section_tags | 標籤 | Tags |
| inbox.tags_empty | 尚未建立標籤 | No tags yet |
| inbox.search_ph | 搜尋寄件者、號碼、備註、標籤、檔名 | Search sender, number, note, tag, file name |
| inbox.sort_newest | 最新 | Newest |
| inbox.sort_oldest | 最舊 | Oldest |
| inbox.sort_sender | 寄件者 | Sender |
| inbox.result_count | 共 {count} 份傳真 | Faxes: {count} |
| inbox.mark_all_read | 全部標為已讀 | Mark all read |
| inbox.empty_trash_btn | 清空垃圾桶 | Empty trash |
| inbox.empty_title | 沒有傳真 | No faxes |
| inbox.empty_trash | 垃圾桶是空的 | Trash is empty |
| inbox.empty_filtered | 沒有符合目前篩選條件的傳真。 | No faxes match the current filters. |
| inbox.empty_no_folder | 尚未設定監看資料夾，請先到「設定」新增。 | No watch folder yet. Add one in Settings. |
| inbox.empty_waiting | 正在等待新傳真，也可以在下方匯入既有檔案。 | Waiting for new faxes. You can also import existing files below. |
| inbox.go_settings | 前往設定 | Open Settings |
| inbox.unknown_sender | 未知寄件者 | Unknown sender |
| inbox.tag_more | +{count} | +{count} |
| inbox.import_ph | 要匯入的傳真檔案或資料夾路徑 | Path of a fax file or folder to import |
| inbox.import_hint | 也可以直接把檔案或資料夾拖曳到視窗中匯入。 | You can also drag files or folders onto this window. |
| inbox.export_ph | 匯出目前篩選結果到此資料夾 | Export filtered faxes to this folder |
| inbox.export_btn | 匯出結果 | Export |

### 11.4 detail
| key | zh-TW | English |
|---|---|---|
| detail.empty_title | 選擇一份傳真 | Select a fax |
| detail.empty_hint | 從清單點選傳真即可在這裡檢視內容。 | Pick a fax from the list to see it here. |
| detail.title | 傳真內容 | Fax details |
| detail.field_received | 接收時間 | Received |
| detail.field_number | 傳真號碼 | Number |
| detail.field_pages | 頁數 | Pages |
| detail.field_size | 檔案大小 | Size |
| detail.field_original | 原始檔名 | Original file |
| detail.field_path | 封存位置 | Archived at |
| detail.in_trash | 已於 {time} 移到垃圾桶 | Moved to trash at {time} |
| detail.mark_read | 標為已讀 | Mark as read |
| detail.mark_unread | 標為未讀 | Mark as unread |
| detail.star | 加星號 | Star |
| detail.unstar | 取消星號 | Unstar |
| detail.open | 開啟 | Open |
| detail.print | 列印 | Print |
| detail.reveal | 在資料夾中顯示 | Show in folder |
| detail.add_contact | 加入聯絡人 | Add to contacts |
| detail.in_contacts | 已在聯絡人中 | Already in contacts |
| detail.trash | 移到垃圾桶 | Move to trash |
| detail.restore | 還原 | Restore |
| detail.delete_forever | 永久刪除 | Delete permanently |
| detail.section_edit | 編輯資訊 | Edit details |
| detail.label_sender | 寄件者名稱 | Sender name |
| detail.label_tags | 標籤 | Tags |
| detail.tags_hint | 以逗號分隔多個標籤 | Separate tags with commas |
| detail.label_note | 備註 | Note |
| detail.save_edits | 儲存變更 | Save changes |
| detail.section_preview | 預覽 | Preview |
| detail.zoom_fit | 符合寬度 | Fit |
| detail.zoom_percent | {percent}% | {percent}% |
| detail.preview_loading | 正在產生預覽… | Rendering preview... |
| detail.preview_unavailable_hint | 無法在此預覽，仍可用預設程式開啟。 | Can't preview here, but you can still open it with the default app. |

### 11.5 contacts
| key | zh-TW | English |
|---|---|---|
| contacts.title | 聯絡人 | Contacts |
| contacts.count | 共 {count} 位聯絡人 | Contacts: {count} |
| contacts.search_ph | 搜尋姓名、公司、號碼 | Search name, company, number |
| contacts.new | 新增聯絡人 | New contact |
| contacts.empty_title | 還沒有聯絡人 | No contacts yet |
| contacts.empty_hint | 新增聯絡人，或從 CSV 檔匯入。 | Add a contact or import from a CSV file. |
| contacts.no_match | 找不到符合的聯絡人 | No matching contacts |
| contacts.send_to | 傳送 | Send |
| contacts.editor_new_title | 新增聯絡人 | New contact |
| contacts.editor_edit_title | 編輯聯絡人 | Edit contact |
| contacts.editor_empty | 選擇一位聯絡人來編輯，或新增一位。 | Select a contact to edit, or create a new one. |
| contacts.field_name | 姓名 | Name |
| contacts.field_company | 公司 | Company |
| contacts.field_fax | 傳真號碼 | Fax number |
| contacts.field_phone | 電話 | Phone |
| contacts.field_email | 電子郵件 | Email |
| contacts.field_note | 備註 | Note |
| contacts.fax_hint | 只會保留數字與開頭的 +。 | Only digits and a leading + are kept. |
| contacts.csv_section | 匯入／匯出 CSV | Import / export CSV |
| contacts.csv_ph | CSV 檔案路徑 | CSV file path |
| contacts.csv_import | 匯入 CSV | Import CSV |
| contacts.csv_export | 匯出 CSV | Export CSV |

### 11.6 send
| key | zh-TW | English |
|---|---|---|
| send.title | 傳送傳真 | Send a fax |
| send.explain_title | 運作方式 | How it works |
| send.explain_body | MyFax 會把文件交給印表機的傳真驅動程式（例如 EPSON FAX）。驅動程式會自己跳出視窗，請在其中輸入傳真號碼並送出。MyFax 無法代為撥號，也無法確認對方是否收到。 | MyFax hands your document to the printer's fax driver (for example EPSON FAX). The driver opens its own window where you enter the fax number and send. MyFax cannot dial for you or confirm delivery. |
| send.section_printer | 1. 傳真印表機 | 1. Fax printer |
| send.section_file | 2. 要傳送的檔案 | 2. File to send |
| send.section_recipient | 3. 收件人（僅供記錄） | 3. Recipient (for your records) |
| send.printer_fax_badge | 傳真 | Fax |
| send.printers_empty | 找不到印表機。請確認已安裝 EPSON FAX 驅動程式，再按「重新整理」。 | No printers found. Make sure the EPSON FAX driver is installed, then press Refresh. |
| send.file_ph | 要傳送的檔案路徑（PDF、圖片、文件…） | File to send (PDF, image, document...) |
| send.recipient_ph | 收件人名稱或傳真號碼（選填） | Recipient name or fax number (optional) |
| send.pick_contact | 從聯絡人選擇 | Pick from contacts |
| send.contacts_empty | 還沒有聯絡人，可到「聯絡人」新增。 | No contacts yet. Add some on the Contacts page. |
| send.note_ph | 備註（選填） | Note (optional) |
| send.button | 傳送傳真 | Send fax |
| send.need_printer | 請先選擇印表機 | Choose a printer first |
| send.need_file | 請先指定要傳送的檔案 | Choose a file to send first |
| send.history_title | 傳送記錄 | Sent history |
| send.history_empty | 還沒有傳送記錄 | Nothing sent yet |
| send.history_via | 經由 {printer} | via {printer} |
| send.history_to | 收件人：{recipient} | To: {recipient} |

### 11.7 stats
| key | zh-TW | English |
|---|---|---|
| stats.title | 統計 | Statistics |
| stats.range_7d | 近 7 天 | 7 days |
| stats.range_30d | 近 30 天 | 30 days |
| stats.range_365d | 近 1 年 | 1 year |
| stats.total_faxes | 傳真總數 | Total faxes |
| stats.total_pages | 總頁數 | Total pages |
| stats.avg_pages | 平均每份頁數 | Avg pages per fax |
| stats.busiest_day | 收件最多的一天 | Busiest day |
| stats.busiest_value | {date}（{count} 份） | {date} ({count}) |
| stats.chart_daily | 每日傳真數量 | Faxes per day |
| stats.chart_monthly | 每月傳真數量 | Faxes per month |
| stats.chart_peak | 最高：{count} | Peak: {count} |
| stats.chart_empty | 此期間沒有傳真 | No faxes in this period |
| stats.chart_range | {from} 至 {to} | {from} to {to} |
| stats.top_senders | 主要寄件者 | Top senders |
| stats.top_empty | 此期間沒有資料 | No data for this period |
| stats.sender_count | {count} 份 | {count} |

### 11.8 log
| key | zh-TW | English |
|---|---|---|
| log.title | 活動記錄 | Activity log |
| log.count | 共 {count} 筆 | Entries: {count} |
| log.clear | 清除記錄 | Clear log |
| log.empty | 還沒有活動記錄 | No log entries yet |
| log.level_info | 資訊 | Info |
| log.level_warn | 警告 | Warning |
| log.level_error | 錯誤 | Error |
| log.truncated | 僅顯示最新 {count} 筆 | Showing the latest {count} |

### 11.9 settings
| key | zh-TW | English |
|---|---|---|
| settings.title | 設定 | Settings |
| settings.section_general | 外觀與語言 | Appearance & language |
| settings.language | 語言 | Language |
| settings.theme | 主題 | Theme |
| settings.theme_light | 淺色 | Light |
| settings.theme_dark | 深色 | Dark |
| settings.section_receive | 接收 | Receiving |
| settings.watch_folders | 監看資料夾 | Watch folders |
| settings.watch_empty | 尚未設定監看資料夾 | No watch folders yet |
| settings.watch_ph | 要新增的監看資料夾路徑 | Folder path to watch |
| settings.poll_interval | 掃描間隔（秒） | Scan interval (seconds) |
| settings.poll_hint | 2 至 3600 秒 | 2 to 3600 seconds |
| settings.opt_delete_source | 封存後刪除來源檔案（移動而非複製） | Delete the source file after archiving (move instead of copy) |
| settings.opt_accept_tiff | 同時接受 TIFF 傳真（.tif / .tiff） | Also accept TIFF faxes (.tif / .tiff) |
| settings.opt_auto_print | 收到新傳真時自動列印 | Print new faxes automatically |
| settings.opt_notify | 收到新傳真時顯示通知橫幅 | Show a banner when a new fax arrives |
| settings.forward_folder | 額外複製到資料夾（選填） | Also copy new faxes to (optional) |
| settings.forward_ph | 每份新傳真的複本會放到這裡 | A copy of every new fax is placed here |
| settings.section_storage | 封存與保留 | Archive & retention |
| settings.archive_root | 封存根目錄 | Archive folder |
| settings.archive_hint | 依「年／年-月／年-月-日」資料夾整理 | Organised as year / year-month / year-month-day folders |
| settings.open_archive | 開啟封存資料夾 | Open archive folder |
| settings.trash_retention | 垃圾桶保留天數 | Keep trash for (days) |
| settings.trash_hint | 0 表示不自動清除 | 0 = never purge automatically |
| settings.page_size | 每頁顯示筆數 | Faxes per page |
| settings.page_hint | 10 至 500 | 10 to 500 |
| settings.save_btn | 儲存設定 | Save settings |
| settings.unsaved | 有尚未儲存的變更 | Unsaved changes |
| settings.apply_note | 語言、主題與開關會立即生效；路徑與數字欄位需按「儲存設定」。 | Language, theme and switches apply immediately; path and number fields need Save settings. |
| settings.err_archive | 請輸入封存資料夾路徑 | Enter an archive folder path |
| settings.err_poll | 請輸入 2 至 3600 的整數 | Enter a whole number from 2 to 3600 |
| settings.err_trash | 請輸入 0 至 3650 的整數 | Enter a whole number from 0 to 3650 |
| settings.err_page | 請輸入 10 至 500 的整數 | Enter a whole number from 10 to 500 |
| settings.help_title | Epson 印表機設定說明 | Epson printer setup |
| settings.help_intro | MyFax 透過印表機把收到的傳真轉存到網路資料夾來接收，請先在印表機上設定一次： | MyFax receives faxes that your printer saves to a network folder. Set this up once on the printer: |
| settings.help_step1 | 1. 在瀏覽器輸入印表機的 IP 位址開啟 Web Config（或使用印表機操作面板），進入傳真的「傳真輸出」設定。 | 1. Open the printer's Web Config by entering its IP address in a browser (or use the control panel), then go to the Fax Output settings. |
| settings.help_step2 | 2. 將「傳真輸出」設為「儲存到網路資料夾」（SMB），路徑填入這台電腦上的共用資料夾，例如 \\PC-NAME\FaxInbox。 | 2. Set Fax Output to Save to Network Folder (SMB) and enter a folder shared from this PC, for example \\PC-NAME\FaxInbox. |
| settings.help_step3 | 3. 檔案格式建議選 PDF；若選 TIFF，請開啟上方的「同時接受 TIFF 傳真」。 | 3. Choose PDF as the file format (recommended). If you pick TIFF, turn on "Also accept TIFF faxes" above. |
| settings.help_step4 | 4. 把同一個資料夾加入上方的「監看資料夾」，再按狀態列的「立即掃描」測試。 | 4. Add the same folder to Watch folders above, then press Scan now in the status bar to test. |
| settings.help_note | 選單名稱依機型而異，請找「Fax Output」或「儲存到網路資料夾」。請勿讓 EPSON FAX Utility 與 MyFax 同時監看同一個資料夾。 | Menu names vary by model; look for Fax Output or Save to Network Folder. Do not let EPSON FAX Utility and MyFax watch the same folder. |

### 11.10 app (controller: toasts, dialogs, log lines, watcher and preview texts)
Toasts (kind in brackets: S success, I info, E error):
| key | zh-TW | English |
|---|---|---|
| app.import_ok | 已匯入 {count} 份傳真 | Imported {count} fax(es) |
| app.import_duplicate | 這份傳真已封存過：{name} | Already archived: {name} |
| app.import_skipped | 略過不支援的檔案：{name} | Skipped unsupported file: {name} |
| app.import_error | 匯入失敗：{error} | Import failed: {error} |
| app.import_summary | 匯入 {imported} 份、重複 {duplicates} 份、略過 {skipped} 份 | Imported {imported}, duplicates {duplicates}, skipped {skipped} |
| app.import_no_path | 請先輸入要匯入的檔案或資料夾路徑 | Enter the file or folder to import first |
| app.export_done | 已匯出 {count} 份傳真到 {folder} | Exported {count} fax(es) to {folder} |
| app.export_none | 目前篩選結果沒有可匯出的傳真 | No faxes to export for the current filter |
| app.export_no_folder | 請先指定匯出資料夾 | Choose an export folder first |
| app.export_error | 匯出失敗：{error} | Export failed: {error} |
| app.print_sent | 已送到印表機：{name} | Sent to the printer: {name} |
| app.print_error | 列印失敗：{error} | Printing failed: {error} |
| app.open_failed | 無法開啟檔案：{error} | Could not open the file: {error} |
| app.reveal_failed | 無法開啟資料夾：{error} | Could not open the folder: {error} |
| app.file_missing | 找不到封存檔案：{path} | Archived file not found: {path} |
| app.edits_saved | 已儲存變更 | Changes saved |
| app.moved_to_trash | 已移到垃圾桶 | Moved to trash |
| app.restored | 已還原 | Restored |
| app.deleted_forever | 已永久刪除 | Deleted permanently |
| app.trash_emptied | 已清空垃圾桶（{count} 份） | Trash emptied ({count}) |
| app.marked_all_read | 已將 {count} 份傳真標為已讀 | Marked {count} fax(es) as read |
| app.scan_done | 掃描完成：{count} 份新傳真 | Scan finished: {count} new fax(es) |
| app.settings_saved | 設定已儲存 | Settings saved |
| app.settings_invalid | 設定未儲存：{reason} | Settings not saved: {reason} |
| app.settings_save_failed | 無法儲存設定：{error} | Could not save settings: {error} |
| app.watch_empty | 請先輸入資料夾路徑 | Enter a folder path first |
| app.watch_missing | 找不到資料夾：{path} | Folder not found: {path} |
| app.watch_duplicate | 這個資料夾已在清單中 | This folder is already in the list |
| app.contact_saved | 已儲存聯絡人 | Contact saved |
| app.contact_deleted | 已刪除聯絡人 | Contact deleted |
| app.contact_invalid_name | 請輸入姓名 | Enter a name |
| app.contact_invalid_fax | 請輸入有效的傳真號碼 | Enter a valid fax number |
| app.contact_added_from_fax | 已將 {name} 加入聯絡人 | Added {name} to contacts |
| app.contact_exists | 這個號碼已在聯絡人中 | This number is already in your contacts |
| app.no_sender_number | 這份傳真沒有寄件者號碼，無法加入聯絡人 | This fax has no sender number, so it can't be added to contacts |
| app.csv_imported | 已匯入 {count} 位聯絡人 | Imported {count} contact(s) |
| app.csv_exported | 已匯出 {count} 位聯絡人到 {path} | Exported {count} contact(s) to {path} |
| app.csv_no_path | 請先指定 CSV 檔案路徑 | Choose a CSV file path first |
| app.csv_error | CSV 處理失敗：{error} | CSV error: {error} |
| app.date_invalid | 日期格式不正確，請輸入 YYYY-MM-DD | Invalid date. Use YYYY-MM-DD. |
| app.date_order | 起始日不能晚於結束日 | The start date can't be after the end date |
| app.send_handed | 已交給傳真驅動程式「{printer}」，請在跳出的視窗輸入傳真號碼並送出。 | Handed to the fax driver "{printer}". Enter the fax number in the window that opens and send. |
| app.send_no_printer | 找不到可用的印表機，請先安裝傳真驅動程式 | No printer available. Install the fax driver first. |
| app.send_file_missing | 找不到要傳送的檔案：{path} | File to send not found: {path} |
| app.send_error | 無法交給傳真驅動程式：{error} | Could not hand the file to the fax driver: {error} |
| app.printers_refreshed | 找到 {count} 台印表機 | Found {count} printer(s) |

Preview (`PreviewStatus::Unavailable(text)` texts) and watcher (`WatcherStatus::Error(text)` texts):
| key | zh-TW | English |
|---|---|---|
| app.preview_unavailable | 無法預覽這份傳真 | This fax can't be previewed |
| app.preview_encrypted | 這個 PDF 已加密，無法預覽 | This PDF is encrypted and can't be previewed |
| app.preview_unsupported | 不支援的檔案格式，無法預覽 | Unsupported format, can't preview |
| app.preview_failed | 預覽失敗：{error} | Preview failed: {error} |
| app.preview_file_missing | 找不到封存檔案，無法預覽 | The archived file is missing, can't preview |
| app.watch_error_folder | 無法讀取資料夾：{path} | Cannot read folder: {path} |
| app.watch_error_start | 無法啟動資料夾監看：{error} | Cannot start the folder watcher: {error} |

Confirm dialogs (title / body built by controller from these; `{name}` = sender display or "common.unknown"):
| key | zh-TW | English |
|---|---|---|
| app.confirm_delete_title | 永久刪除這份傳真？ | Delete this fax permanently? |
| app.confirm_delete_body | 「{name}」及其封存檔案將被永久刪除，無法復原。 | "{name}" and its archived file will be deleted permanently. This cannot be undone. |
| app.confirm_empty_title | 清空垃圾桶？ | Empty the trash? |
| app.confirm_empty_body | 垃圾桶中的 {count} 份傳真將被永久刪除，無法復原。 | The {count} fax(es) in the trash will be deleted permanently. This cannot be undone. |
| app.confirm_contact_title | 刪除這位聯絡人？ | Delete this contact? |
| app.confirm_contact_body | 「{name}」將從聯絡人中刪除。 | "{name}" will be removed from your contacts. |
| app.confirm_log_title | 清除活動記錄？ | Clear the activity log? |
| app.confirm_log_body | 所有活動記錄都會被刪除，無法復原。 | All log entries will be deleted. This cannot be undone. |

Log lines (stored in the language active when written):
| key | zh-TW | English |
|---|---|---|
| app.log_started | MyFax 已啟動 | MyFax started |
| app.log_new_fax | 收到新傳真：{name}（{pages} 頁） | New fax received: {name} ({pages} pg) |
| app.log_imported | 已匯入傳真：{name} | Imported fax: {name} |
| app.log_import_duplicate | 略過重複的傳真：{name} | Skipped duplicate fax: {name} |
| app.log_import_error | 匯入失敗：{name}（{error}） | Import failed: {name} ({error}) |
| app.log_exported | 已匯出 {count} 份傳真到 {folder} | Exported {count} fax(es) to {folder} |
| app.log_printed | 已列印：{name} | Printed: {name} |
| app.log_trashed | 已移到垃圾桶：{name} | Moved to trash: {name} |
| app.log_restored | 已還原：{name} | Restored: {name} |
| app.log_deleted | 已永久刪除：{name} | Deleted permanently: {name} |
| app.log_trash_emptied | 已清空垃圾桶（{count} 份） | Emptied the trash ({count}) |
| app.log_trash_purged | 已自動清除垃圾桶中逾期的 {count} 份傳真 | Purged {count} expired fax(es) from the trash |
| app.log_settings_saved | 已儲存設定 | Settings saved |
| app.log_watch_added | 已新增監看資料夾：{path} | Added watch folder: {path} |
| app.log_watch_removed | 已移除監看資料夾：{path} | Removed watch folder: {path} |
| app.log_watcher_started | 開始監看 {count} 個資料夾 | Watching {count} folder(s) |
| app.log_watcher_error | 資料夾監看錯誤：{error} | Folder watcher error: {error} |
| app.log_scan_manual | 手動掃描完成，發現 {count} 份新傳真 | Manual scan finished: {count} new fax(es) |
| app.log_contact_saved | 已儲存聯絡人：{name} | Saved contact: {name} |
| app.log_contact_deleted | 已刪除聯絡人：{name} | Deleted contact: {name} |
| app.log_csv_imported | 已從 CSV 匯入 {count} 位聯絡人 | Imported {count} contact(s) from CSV |
| app.log_csv_exported | 已匯出 {count} 位聯絡人到 CSV | Exported {count} contact(s) to CSV |
| app.log_send_handed | 已交給傳真驅動程式：{file} → {printer} | Handed to fax driver: {file} → {printer} |

---------------------------------------------------------------------------------------------------

## 12. Interaction acceptance (run by a verifier after implementation; each is Given / When / Then)

1. Shell: 1024x680, Inbox. Then every nav item visible, status bar visible, no clipped panes, list >= 280 px wide. When window is 1280x800, the list grows, the filter and detail panes keep 224 and 352.
2. Glyph sheet: temporarily render every `glyph::*` once in both themes; each draws a visible glyph (no tofu). Else swap the const.
3. Nav: click Contacts -> `Navigate(Contacts)`; active item shows Accent bg + Primary bar; Inbox badge equals `counts.unread` (`99+` above 99, hidden at 0).
4. Banner: `new_fax_banner = 3` on Settings -> banner "3 new faxes" with View and x; View -> Inbox and banner gone; x -> gone and page stays.
5. Toast: Success toast appears bottom centre above the status bar, does not block clicks on the page beneath; click toast -> `DismissToast`.
6. Dialog: `AskEmptyTrash` -> scrim + centred dialog (420 wide) at both 1024x680 and 1280x800; clicking page content beneath does nothing; Esc -> `ConfirmNo`; Delete button -> `ConfirmYes`.
7. Filters: click "Today" -> `SetDatePreset(Today)`, chip turns Primary, others Secondary. Type `2026-03-01`/`2026-03-31`, Apply (or Enter) -> `ApplyCustomRange`; invalid text -> error toast `app.date_invalid`.
8. Date tree: click chevron -> `ToggleYear`; click year text -> `SelectYear`; click month -> `SelectMonth` and its days appear (only if `expanded_month` matches); click day -> `SelectDay`; the active node highlights per the derivation rule.
9. Tags: click `#invoice` -> `SetTagFilter(Some("invoice"))` and button turns Primary; click again -> `SetTagFilter(None)`.
10. List: unread row shows `●` and SemiBold sender; starred shows `★`; unknown sender shows "Unknown sender"; time reads `2026-03-05 14:07`; a 60-char CJK sender is cut with `…` and never overflows the row.
11. Selected row: click row -> `SelectFax(id)`; row gets Accent bg, bar, `AccentForeground` text; text does not shift.
12. Pager: page 1 of 3 -> Previous disabled, text "1 / 3"; Next -> `NextPage`; last page -> Next disabled. `total == 0` -> "1 / 1" both disabled.
13. Empty states: four list variants and the detail variant appear per the table in 3.4 (first match wins).
14. Trash folder: header shows "Empty trash" (Danger) instead of "Mark all read"; detail shows Restore and Delete permanently, plus the "Moved to trash at ..." strip.
15. Detail edit: fields show sender/tags/note of the selected fax; edit tags, Enter or Save -> `SaveFaxEdits(id)`.
16. Preview: `Ready`, 3 pages, zoom 1.0 -> slot width 328, correct ratio, page "1 / 3"; ZoomIn x1 -> 410 px wide, viewport scrolls; ZoomOut at 0.5 -> disabled; `Unavailable` -> message + Open button -> `OpenExternal`; `Empty` with a selection -> "Rendering preview...".
17. Import/export: type path, Enter or Import -> `ImportFromField`; Browse -> `Browse(Field::ImportPath)`; export disabled when no results.
18. Contacts: search "wang" filters rows view-side; click row -> `EditContact`, editor shows Delete only for id > 0; New -> `NewContact` with title "New contact"; Send on a row -> `UseContactForSend`; Delete -> `AskDeleteContact`; CSV import/export buttons emit `ImportContactsCsv` / `ExportContactsCsv`.
19. Send: printers list shows fax printers first with a "Fax" badge; selecting -> `SelectPrinter`; with no printer or no file the warning captions show and `SendFax` still emits and the toast explains; history rows list newest first as given.
20. Stats: 30-day mode draws 30 bars, labels every 5th; 7-day shows counts above bars; 365 shows 12 months with `MM`; zero days show a 2 px Border stub; total 0 -> "No faxes in this period"; avg shows "—" when total is 0.
21. Log: levels use success/warning/danger badges; `Clear log` disabled when empty and emits `AskClearLog`.
22. Settings: language buttons show native names and emit `SetLanguage`; all seven path/number fields round-trip; entering `1` in Scan interval shows the red message and "Unsaved changes"; Save -> `SaveSettings`; toggles show ☑/☐ and emit `ToggleSetting`.
23. Language switch: change to English -> every string on every page switches, none shows a raw key; switch back.
24. Themes: Light and Dark both legible on all pages (no text on Accent/Muted that fails contrast); no hard-coded colours (`rg "Direct\(|0x[0-9a-f]{6}|rgb"` in `src/ui/views` is empty).
25. i18n gate: `cargo test --lib i18n` passes (keys unique, placeholders match, every used key exists).

Status of this section: NOT_RUN (spec only, no build or render was performed).

---------------------------------------------------------------------------------------------------

## 13. OPEN items for the orchestrator

1. Runtime contract (0.3 A4): please add to `runtime/mod.rs` docs "a container whose key starts with `modal:` swallows pointer events". Fallback if refused: scrim `on_click(ConfirmNo)` and dialog card `on_click(DismissToast)` (harmless no-op while a dialog is open).
2. Runtime contract (A2): non-clickable containers must not block hit-tests (needed for the full-window toast layer). Fallback: toast layer sized to the toast only (cannot be bottom-centred without spacers, so the toast would sit top-left of the stack).
3. Runtime contract (A1/A3/A5): hover overlay, intrinsic button width, focus ring.
4. Controller must supply: field priming (10.1), banner reset, toast timers, Submit map, `UseContactForSend` text format, confirm-gating. If any is declined, tell the views workers before wave 1.
5. `LogLevel` has no Success; Info rendered with the success tone (see 7).
6. `inbox.import_hint` is the only drag-and-drop UI; the platform layer must route dropped paths to the import path.
7. Whether permanent delete removes the archived file from disk is assumed yes in `app.confirm_delete_body`; adjust the text if the archive worker keeps the file.
8. Contacts CSV column format is not stated anywhere in the UI text on purpose (unknown); add it to `contacts.csv_section` once the contacts module fixes it.

## 變更歷史
| 版本 | 日期 | 內容 | 影響範圍 |
|------|------|------|----------|
| v1.0 | 2026-11-10 | 初版 UI 規格（殼層、五頁加設定、i18n 表、驗收例） | docs/team/myfax/UI_SPEC.md |
