# Pane UI Kit — Build Specification

> **For the implementing agent.** This file is the single source of truth for building **Pane**, a macOS‑inspired design system for the web, as a React component kit on top of headless base primitives. Read §0 completely before writing code. Every value you need is in this file. Do not invent colors, sizes, radii, shadows or copy conventions that are not here.

Spec version 1.0 · 24 October 2026 · generated from the Pane design system (tokens v1, 43 colors × 2 themes, 22 components, 32 icons)

---

## Contents

0. Brief for the agent
1. The design language in one page
2. Architecture and conventions
3. Tokens
4. Global rules: states, focus, motion, materials, type, copy, accessibility
5. Component specifications (22)
6. Composition recipes
7. Extending Pane beyond the 22 components
8. Acceptance checklist
- Appendix A: `tokens.css` (drop-in, verbatim)
- Appendix B: Icon set (path data and component contract)
- Appendix C: Reference stylesheet from the Pane prototype

---

## 0. Brief for the agent

### 0.1 Goal

Build a production React component library ("the kit") that reproduces Pane's look and behavior exactly. Use headless **base components** for behavior, keyboard and accessibility. Use Pane's tokens and this spec for every visual decision.

### 0.2 Deliverables

1. `tokens.css`: Appendix A verbatim. It is the only place raw color values live.
2. `base.css`: page defaults (§2.4).
3. `icons`: the 32 icons from Appendix B, plus an `<Icon>` component.
4. The 22 components in §5, each with typed props, styles and a demo/story showing every variant and state in **both themes**.
5. A theme switcher utility (`setTheme('light' | 'dark' | 'system')`) as described in §2.3.
6. Tests: keyboard interaction for every interactive component, plus an automated contrast check of the pairs in §3.8.

### 0.3 Ground rules (MUST)

1. **Tokens only.** Component styles reference CSS custom properties from `tokens.css` (`var(--accent-fill)`). No hex, rgb or named colors in component styles. The only exceptions are listed in §2.6.
2. **Behavior comes from the primitive.** Never re-implement focus trapping, roving tabindex, typeahead, positioning, collision handling, dismissal or ARIA wiring that the base library provides. Style the primitive's state attributes (`[data-checked]`, `[data-highlighted]`, `[data-state="open"]`) and do not mirror them in React state.
3. **Exact metrics.** Copy sizes exactly. 5px stays 5px and 22px stays 22px. Do not snap values to a 4px grid or swap them for a library's defaults.
4. **Both themes, every component.** Light is the default theme and dark is defined in `tokens.css`. Never branch on the theme in JavaScript.
5. **Accessible by default.** Keep a visible focus ring on `:focus-visible`, support keyboard operation, provide an accessible name for every control, and honor `prefers-reduced-motion`.
6. **Mac conventions.** Use `cursor: default` on controls (not `pointer`). Use title-case commands and an ellipsis (`…`) for commands that need more input. Allow at most one primary (default) button per window or dialog.
7. **Don't ship Apple assets.** No SF Pro font files, no SF Symbols, no Apple logos. Use the system font stack and the Pane icon set.

### 0.4 Assumptions (adjust if your codebase differs)

| Topic | Assumption | If different |
|---|---|---|
| Framework | React 18.3+ or 19, TypeScript | Keep the component APIs; port the markup |
| Base primitives | **Base UI**, `@base-ui/react` v1.x, with per-component imports (`import { Menu } from '@base-ui/react/menu'`) | Each spec lists the **Radix Primitives** equivalent. If you have your own base components, treat each "Build on" line as the behavior contract they must satisfy. |
| Styling | Plain CSS consuming CSS variables (CSS Modules or one global stylesheet with the `pn-` prefix) | Tailwind: see §3.9. CSS-in-JS: reference the same variables. |
| Fonts | System stack (`--font-sans`, `--font-mono`) | Do not self-host SF Pro |
| Icons | Pane's own 16px line icons (Appendix B) | Any replacement must match the 16px grid, 1.5px stroke and round caps |

Before styling, confirm data-attribute names against the installed primitive's docs. Base UI uses presence attributes (`data-checked`, `data-highlighted`, `data-disabled`, `data-popup-open`, `data-starting-style`, `data-ending-style`). Radix uses `data-state="checked|unchecked|open|closed|on|off"` plus `data-highlighted` and `data-disabled`.

### 0.5 Definition of done

All items in §8 pass. Every component demo matches its visual spec in both themes and works with the keyboard alone.

---

## 1. The design language in one page

Pane brings the modern Mac desktop (the Big Sur through Sequoia era) to the web:

- **Content first, chrome recedes.** Windows are mostly white (`content-bg`). Toolbars, sidebars and menus are translucent **materials** that blur what sits behind them. Color is reserved for one accent (blue) and for meaning.
- **Small and precise.** Body text is 13px, controls are 22px tall, controls have 5px corners and windows 10px, separators are hairlines. Density is intentional.
- **Elevation is a hairline plus a soft blur.** Controls sit on a 0.5px rim with a tiny shadow. Menus and windows float on large soft shadows. Dark mode adds a faint top highlight.
- **Familiar behavior.** The default button responds to Return and Cancel to Escape. Arrow keys work inside groups. Menus show keyboard shortcuts with the ⌃ ⌥ ⇧ ⌘ glyphs. Sidebar icons are tinted with the accent.
- **Accessible where macOS isn't.** Pane deliberately darkens secondary text, strengthens field outlines and fills the destructive button so every text pair meets WCAG AA in both themes. Keep these deviations. They are intentional.

---

## 2. Architecture and conventions

### 2.1 Suggested layout

```
pane/
  styles/
    tokens.css          ← Appendix A, verbatim
    base.css            ← §2.4
  icons/
    paths.ts            ← Appendix B data
    Icon.tsx
  components/
    Button/Button.tsx, Button.module.css, index.ts
    …one folder per component in §5
  index.ts              ← re-exports everything
```

### 2.2 Component conventions

- `forwardRef` (or the React 19 `ref` prop) on every component. Spread remaining props onto the primitive's root and merge `className`.
- Expose variants as `data-*` attributes on the root (`data-variant="primary"`, `data-size="small"`) and style those. Don't concatenate class names.
- **Sizes:** `size?: 'small' | 'regular' | 'large'`, default `regular`. Map each size to the size tokens (§3.6). Use one size per row of controls.
- **Controlled and uncontrolled** state follows the base library's naming: `value`, `defaultValue`, `onValueChange`; `checked`, `defaultChecked`, `onCheckedChange`; `open`, `defaultOpen`, `onOpenChange`. (The Pane prototype used `onChange`. The kit standardizes on the primitive names.)
- Pass composition props (`render` for Base UI, `asChild` for Radix) through so consumers can swap the rendered element, for example a `Button` rendered as a link.
- Never set `cursor: pointer` on controls. Links (`<a>`) keep the pointer.

### 2.3 Theming

- Themes are set by `data-theme="light" | "dark"` on `<html>`. With no attribute, `tokens.css` follows `prefers-color-scheme`. To force light, set `data-theme="light"`.
- Put `data-theme` on `<html>` only. Popups render in portals at the end of `<body>`, so a theme set on a subtree would not reach them.
- `color-scheme` is set by `base.css` so native scrollbars, date pickers and `<select>` lists match.
- **Accent re-theming:** override three tokens together, e.g. for purple: `--accent: var(--purple); --accent-fill: <a darker purple with ≥4.5:1 against white>; --link: <a purple with ≥4.5:1 on window-bg>`. Re-check §3.8 afterwards.

### 2.4 `base.css`

```css
:root, [data-theme="light"] { color-scheme: light; }
[data-theme="dark"] { color-scheme: dark; }
@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) { color-scheme: dark; } }

body {
  margin: 0;
  font-family: var(--font-sans);
  font-size: 13px;
  line-height: 16px;
  background: var(--window-bg);
  color: var(--label);
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}
```

### 2.5 Materials (translucent surfaces)

`toolbar-bg`, `sidebar-bg` and `menu-bg` are semi-transparent. Wherever they are used, always add:

```css
background: var(--sidebar-bg); /* or toolbar-bg / menu-bg */
-webkit-backdrop-filter: blur(var(--blur-material)) saturate(var(--material-saturation));
backdrop-filter: blur(var(--blur-material)) saturate(var(--material-saturation));
```

The tokens are 85–92% opaque, so they stay legible where `backdrop-filter` is unsupported, and no fallback is needed. Never stack a material on another material. Inactive windows drop the blur.

### 2.6 Permitted literal values

Literals are allowed only for:

- Geometry that belongs to one component and is listed in its spec (e.g. the 1px segment padding, the 6px search icon inset).
- `transparent`, `currentColor`, `inherit`.
- The pressed-state filter `brightness(0.9)` on primary and destructive buttons.

---

## 3. Tokens

All tokens are CSS custom properties defined in Appendix A. Names are stable: use them exactly.

### 3.1 Color (light | dark)

Translucent values are 8-digit hex (`#rrggbbaa`). `{name}` is an alias of another token.

| Token | Light | Dark | Usage |
|---|---|---|---|
| `window-bg` | `#ececec` | `#323232` | Window background: behind toolbars, form groups, alerts and the page itself. |
| `content-bg` | `#ffffff` | `#1e1e1e` | Content areas: document canvases, table and list bodies, grouped form boxes. |
| `content-bg-alt` | `#f4f5f5` | `#282828` | Every second row of a table or list on content-bg (zebra striping). |
| `toolbar-bg` | `#f6f6f6eb` | `#363636eb` | Unified title bar and toolbar; translucent, blurred with blur-material. |
| `sidebar-bg` | `#e8e8e8d9` | `#282828d9` | Sidebar (source list) material; translucent, blurred with blur-material. |
| `menu-bg` | `#f2f2f2eb` | `#2c2c2ceb` | Menus, pop-up lists and help tags (tooltips); translucent, blurred with blur-material. |
| `control-bg` | `#ffffff` | `#636366` | Push buttons, the selected segment and pop-up buttons at rest. |
| `control-bg-pressed` | `#e3e3e3` | `#4f4f51` | Push buttons and pop-up buttons while pressed. |
| `field-bg` | `#ffffff` | `#2c2c2e` | Text fields, search fields and other editable wells. |
| `fill` | `#0000000d` | `#ffffff12` | Hover wash behind toolbar buttons, sidebar rows and menu rows; the slider and progress track. |
| `fill-strong` | `#0000001a` | `#ffffff1f` | Pressed toolbar buttons, the segmented-control track and the selected sidebar row. |
| `group-bg` | `#0000000a` | `#ffffff0d` | Grouped form boxes (System Settings style), placed on content-bg. |
| `knob` | `#ffffff` | `#e5e5e7` | Switch and slider knobs. |
| `selection-inactive` | `#dcdcdc` | `#3a3a3a` | Selected table row when its table does not have keyboard focus. |
| `segment-selected` | `{control-bg}` | `{control-bg}` | The selected segment of a segmented control (alias of control-bg). |
| `separator` | `#0000001a` | `#ffffff1a` | Decorative hairlines between rows, toolbar edges and table headers. Not for control outlines. |
| `control-border` | `#808085` | `#8e8e93` | Outline of text fields, checkboxes, radios and the off switch: 3:1 on window-bg, content-bg and field-bg in both themes. |
| `label` | `#000000d9` | `#ffffffd9` | Primary text and glyphs, on every surface: window-bg, content-bg, control-bg, field-bg and the materials. |
| `label-secondary` | `#00000099` | `#ffffff8c` | Secondary text, placeholders, shortcuts and metadata, on window-bg, content-bg, field-bg and the materials. Darker than macOS's 50% so it holds 4.5:1. |
| `label-tertiary` | `#00000047` | `#ffffff40` | Disabled labels and glyphs only (inactive controls are exempt from contrast minimums). Never for content. |
| `accent` | `{blue}` | `{blue}` | The control accent (alias of blue): checked checkboxes and radios, the on switch, slider fill, progress, focus. A mark, not text — use link for text. |
| `accent-fill` | `#0063e1` | `#0058d0` | Fills that carry text: the default (primary) button, highlighted menu item, focused table selection, accent badge. |
| `on-accent` | `#ffffff` | `#ffffff` | Text and glyphs on accent-fill and on the accent marks (checkmark, radio dot). |
| `link` | `#0066cc` | `#4aa3ff` | Links and accent-coloured text, on window-bg, content-bg and the materials. |
| `red` | `#ff3b30` | `#ff453a` | System red as a fill: status dots, tags, chart series. Never text; pair with a word or icon. |
| `orange` | `#ff9500` | `#ff9f0a` | System orange as a fill: status dots, tags, chart series. Never text. |
| `yellow` | `#ffcc00` | `#ffd60a` | System yellow as a fill: tags, stars, chart series. Never text. |
| `green` | `#28cd41` | `#32d74b` | System green as a fill: status dots, tags, chart series. Never text; pair with a word or icon. |
| `blue` | `#007aff` | `#0a84ff` | System blue; the default accent. As a fill or mark only (3:1), never body text. |
| `purple` | `#af52de` | `#bf5af2` | System purple as a fill: tags, chart series. Never text. |
| `pink` | `#ff2d55` | `#ff375f` | System pink as a fill: tags, chart series. Never text. |
| `gray` | `#8e8e93` | `#98989d` | System gray as a fill: neutral tags, inactive status dots. Never text. |
| `red-text` | `#d70015` | `#ff6961` | Error and destructive text, on window-bg, content-bg, content-bg-alt, field-bg and menu-bg (not on control-bg). Always with a word or icon, never colour alone. |
| `orange-text` | `#b83000` | `#ffb340` | Warning text, on window-bg, content-bg, content-bg-alt, field-bg and menu-bg (not on control-bg). |
| `green-text` | `#1a7431` | `#30db5b` | Success text, on window-bg, content-bg, content-bg-alt, field-bg and menu-bg (not on control-bg). Always with a word or icon, never colour alone. |
| `danger-fill` | `#d70015` | `#d92418` | Fill of the destructive button and the red count badge. |
| `on-danger` | `#ffffff` | `#ffffff` | Text on danger-fill. |
| `window-close` | `#ff5f57` | `#ff5f57` | Close button of the window controls (traffic lights), active window. |
| `window-minimize` | `#febc2e` | `#febc2e` | Minimize button of the window controls, active window. |
| `window-zoom` | `#28c840` | `#28c840` | Zoom (full-screen) button of the window controls, active window. |
| `window-control-inactive` | `#d1d1d1` | `#4e4e4e` | All three window controls when the window is inactive. |
| `window-control-rim` | `#00000026` | `#0000004d` | 0.5px rim around each window control. |
| `window-control-glyph` | `#0000008c` | `#0000008c` | The ×, − and ↗ glyphs drawn on the controls on hover. |

**Color rules**

- Text uses only `label`, `label-secondary`, `link`, `on-accent`, `on-danger`, and the `*-text` status colors. `label-tertiary` is for disabled UI only.
- The hues `red` … `gray` are fills: tags, status dots, chart series, icon tiles. They are never used as text.
- Status always carries a word or icon as well as its color.
- Use `accent` for marks without text (check, switch, slider, sidebar icons). Use `accent-fill` for fills that carry text.

### 3.2 Typography

Families: `--font-sans` = `-apple-system, BlinkMacSystemFont, "SF Pro Text", "SF Pro", "Helvetica Neue", "Segoe UI", system-ui, sans-serif`; `--font-mono` = `ui-monospace, "SF Mono", SFMono-Regular, Menlo, Monaco, Consolas, monospace`.

| Style | Size / line-height | Weight | Family | Class | Use for |
|---|---|---|---|---|---|
| `large-title` | 26px / 32px | 400 | `--font-sans` | `.pn-text-large-title` | One per window at most: the title of a full-window view. |
| `title-1` | 22px / 26px | 400 | `--font-sans` | `.pn-text-title-1` | Page titles inside a settings pane or document. |
| `title-2` | 17px / 22px | 400 | `--font-sans` | `.pn-text-title-2` | Section titles. |
| `title-3` | 15px / 20px | 400 | `--font-sans` | `.pn-text-title-3` | Sub-section titles and inspector headers. |
| `headline` | 13px / 16px | 700 | `--font-sans` | `.pn-text-headline` | Alert titles, emphasised row titles, group-box titles. |
| `body` | 13px / 16px | 400 | `--font-sans` | `.pn-text-body` | Everything by default: control labels, rows, menu items, form text. |
| `callout` | 12px / 15px | 400 | `--font-sans` | `.pn-text-callout` | Descriptions under a row title; help tags. |
| `subheadline` | 11px / 14px | 400 | `--font-sans` | `.pn-text-subheadline` | Small-control labels and secondary metadata. |
| `footnote` | 10px / 13px | 400 | `--font-sans` | `.pn-text-footnote` | Fine print beneath a group box. |
| `caption-1` | 10px / 13px | 400 | `--font-sans` | `.pn-text-caption-1` | Captions under thumbnails and icons. |
| `caption-2` | 10px / 13px | 500 | `--font-sans` | `.pn-text-caption-2` | Sidebar section headers (uppercase is optional) and badges. |
| `code` | 12px / 16px | 400 | `--font-mono` | `.pn-text-code` | Code, paths and terminal output. |

Appendix A ships each style as a class (`.pn-text-body`, `.pn-text-headline`, …). Components set their own font metrics as specified in §5. Use `font-variant-numeric: tabular-nums` for numbers in columns, badges and progress values.

### 3.3 Spacing

4px base; 20px window margins, 8px between related controls, as in macOS layouts.

| Token | Value | Usage |
|---|---|---|
| `space-1` | `4px` | Icon-to-label gap; padding inside badges. |
| `space-2` | `8px` | Between related controls in a row; menu and sidebar row insets. |
| `space-3` | `12px` | Between a label and its control; group-box row padding. |
| `space-4` | `16px` | Alert and popover padding; between control groups. |
| `space-5` | `20px` | Window content margin (the standard macOS edge inset). |
| `space-6` | `24px` | Between stacked group boxes. |
| `space-8` | `32px` | Above a new section in long settings panes. |

### 3.4 Radius

Small and precise: controls 5px, menus 6px, windows 10px; only switches and badges are fully round.

| Token | Value | Usage |
|---|---|---|
| `radius-xs` | `3px` | Checkboxes and the pop-up button's chevron well. |
| `radius-sm` | `5px` | Push buttons, text fields, segmented controls, toolbar buttons, selected rows. |
| `radius-md` | `6px` | Menus, help tags, search fields, sidebar selection. |
| `radius-lg` | `10px` | Windows, alerts, sheets and grouped form boxes. |
| `radius-full` | `9999px` | Switches, radio buttons, count badges, slider knobs. |

### 3.5 Shadow

Hairline-plus-blur elevations. Dark mode adds a faint top highlight and deepens the blur.

| Token | Light | Dark | Usage |
|---|---|---|---|
| `shadow-control` | `0 0 0 0.5px #0000001f, 0 0.5px 1.5px #00000033` | `inset 0 0.5px 0 #ffffff26, 0 0 0 0.5px #00000066, 0 0.5px 1.5px #00000066` | Push buttons, pop-up buttons and the selected segment at rest. |
| `shadow-field` | `inset 0 0.5px 1px #0000001a` | `inset 0 0.5px 1px #00000040` | The inset of text and search fields (with control-border). |
| `shadow-knob` | `0 0 0 0.5px #00000029, 0 1px 2.5px #00000033` | `0 0 0 0.5px #00000066, 0 1px 2.5px #00000080` | Switch and slider knobs. |
| `shadow-menu` | `0 0 0 0.5px #0000002e, 0 10px 32px #00000038` | `inset 0 0 0 0.5px #ffffff26, 0 0 0 0.5px #000000b3, 0 10px 32px #00000099` | Menus, pop-up lists and help tags. |
| `shadow-window` | `0 0 0 0.5px #00000040, 0 4px 12px #0000001f, 0 22px 56px #00000047` | `inset 0 0 0 0.5px #ffffff26, 0 0 0 0.5px #000000d9, 0 22px 56px #000000a6` | Windows and alerts floating over the page. |
| `focus-ring` | `0 0 0 3px #007aff` | `0 0 0 3px #0a84ff` | Keyboard focus: a solid 3px accent ring that hugs the control's radius (box-shadow). 3:1 on every surface. |

### 3.6 Size

Control and chrome metrics from macOS (regular size unless named).

| Token | Value | Usage |
|---|---|---|
| `control-small` | `18px` | Height of small push buttons, fields and segmented controls (11px labels). |
| `control-regular` | `22px` | Height of regular push buttons, fields, pop-ups and segmented controls (13px labels). |
| `control-large` | `28px` | Height of large controls and toolbar buttons. |
| `row-height` | `24px` | Table and list rows; menu items are 22px. |
| `sidebar-row` | `28px` | Sidebar (source list) rows. |
| `titlebar-height` | `28px` | A title bar with no toolbar. |
| `toolbar-height` | `52px` | A unified title bar and toolbar. |
| `sidebar-width` | `220px` | Default sidebar width. |

### 3.7 Material and opacity

| Token | Value | Usage |
|---|---|---|
| `blur-material` | `24px` | Backdrop blur behind sidebar-bg, toolbar-bg and menu-bg. |
| `material-saturation` | `180%` | Backdrop saturate() that keeps colour vivid through a material. |
| `opacity-disabled` | `0.45` | Whole disabled controls (fills and borders); labels switch to label-tertiary. |

### 3.8 Verified contrast pairs

These pairs were checked with the WCAG 2 formula. Translucent colors are composited onto their real ground: materials over `window-bg`, `group-bg` over `content-bg`. The figure shown is the **minimum across the listed surfaces**. Your automated test must reproduce these minimums: text ≥ 4.5:1, marks, borders and focus ≥ 3:1.

| Foreground | On | Min. required | Light (min) | Dark (min) | Used for |
|---|---|---|---|---|---|
| `label` | `window-bg`, `content-bg`, `content-bg-alt`, `toolbar-bg`, `sidebar-bg`, `menu-bg`, `control-bg`, `control-bg-pressed`, `field-bg`, `selection-inactive`, `group-bg`, `fill-strong` over `sidebar-bg` | 4.5:1 | 10.77 | 4.88 | Primary text |
| `label-secondary` | `window-bg`, `content-bg`, `content-bg-alt`, `toolbar-bg`, `sidebar-bg`, `menu-bg`, `field-bg`, `group-bg`, `selection-inactive` | 4.5:1 | 5.19 | 4.68 | Secondary text, placeholders, shortcuts |
| `link` | `window-bg`, `content-bg`, `content-bg-alt`, `toolbar-bg`, `sidebar-bg`, `menu-bg`, `group-bg` | 4.5:1 | 4.57 | 4.61 | Links |
| `red-text` | `window-bg`, `content-bg`, `content-bg-alt`, `field-bg`, `menu-bg` | 4.5:1 | 4.56 | 4.55 | Error text |
| `orange-text` | `window-bg`, `content-bg`, `content-bg-alt`, `field-bg`, `menu-bg` | 4.5:1 | 5.14 | 7.19 | Warning text |
| `green-text` | `window-bg`, `content-bg`, `content-bg-alt`, `field-bg`, `menu-bg` | 4.5:1 | 4.96 | 6.97 | Success text |
| `on-accent` | `accent-fill` | 4.5:1 | 5.42 | 6.35 | Primary button, menu highlight, focused selection |
| `on-danger` | `danger-fill` | 4.5:1 | 5.38 | 4.98 | Destructive button, danger badge |
| `on-accent` | `accent` | 3.0:1 | 4.02 | 3.65 | Checkmark and radio dot (mark) |
| `on-accent` | `blue`, `purple`, `red`, `pink` | 3.0:1 | 3.55 | 3.41 | FormRow icon-tile glyphs (mark) |
| `accent` | `window-bg`, `content-bg`, `content-bg-alt`, `field-bg`, `toolbar-bg`, `sidebar-bg`, `menu-bg`, `group-bg` | 3.0:1 | 3.30 | 3.33 | Accent marks and the focus ring (`focus-ring` uses the accent values) |
| `control-border` | `window-bg`, `content-bg`, `field-bg`, `sidebar-bg`, `group-bg` | 3.0:1 | 3.22 | 3.93 | Field, checkbox, radio and off-switch outlines |

### 3.9 Optional: Tailwind v4 mapping

If the codebase uses Tailwind v4, map the tokens in your main CSS so utilities such as `bg-window-bg`, `text-label`, `rounded-pn-sm` and `shadow-pn-control` resolve to Pane variables:

```css
@import "tailwindcss";
@import "./pane/styles/tokens.css";

@theme inline {
  /* colors → bg-window-bg, text-label, border-control-border … */
  --color-window-bg: var(--window-bg);
  --color-content-bg: var(--content-bg);
  --color-content-bg-alt: var(--content-bg-alt);
  --color-toolbar-bg: var(--toolbar-bg);
  --color-sidebar-bg: var(--sidebar-bg);
  --color-menu-bg: var(--menu-bg);
  --color-control-bg: var(--control-bg);
  --color-control-bg-pressed: var(--control-bg-pressed);
  --color-field-bg: var(--field-bg);
  --color-fill: var(--fill);
  --color-fill-strong: var(--fill-strong);
  --color-group-bg: var(--group-bg);
  --color-knob: var(--knob);
  --color-selection-inactive: var(--selection-inactive);
  --color-segment-selected: var(--segment-selected);
  --color-separator: var(--separator);
  --color-control-border: var(--control-border);
  --color-label: var(--label);
  --color-label-secondary: var(--label-secondary);
  --color-label-tertiary: var(--label-tertiary);
  --color-accent: var(--accent);
  --color-accent-fill: var(--accent-fill);
  --color-on-accent: var(--on-accent);
  --color-link: var(--link);
  --color-red: var(--red);
  --color-orange: var(--orange);
  --color-yellow: var(--yellow);
  --color-green: var(--green);
  --color-blue: var(--blue);
  --color-purple: var(--purple);
  --color-pink: var(--pink);
  --color-gray: var(--gray);
  --color-red-text: var(--red-text);
  --color-orange-text: var(--orange-text);
  --color-green-text: var(--green-text);
  --color-danger-fill: var(--danger-fill);
  --color-on-danger: var(--on-danger);
  --color-window-close: var(--window-close);
  --color-window-minimize: var(--window-minimize);
  --color-window-zoom: var(--window-zoom);
  --color-window-control-inactive: var(--window-control-inactive);
  --color-window-control-rim: var(--window-control-rim);
  --color-window-control-glyph: var(--window-control-glyph);
  /* radii → rounded-pn-sm … */
  --radius-pn-xs: var(--radius-xs);
  --radius-pn-sm: var(--radius-sm);
  --radius-pn-md: var(--radius-md);
  --radius-pn-lg: var(--radius-lg);
  --radius-pn-full: var(--radius-full);
  /* shadows → shadow-pn-control … (focus ring: shadow-pn-focus-ring) */
  --shadow-pn-control: var(--shadow-control);
  --shadow-pn-field: var(--shadow-field);
  --shadow-pn-knob: var(--shadow-knob);
  --shadow-pn-menu: var(--shadow-menu);
  --shadow-pn-window: var(--shadow-window);
  --shadow-pn-focus-ring: var(--focus-ring);
  /* fonts → font-pane, font-pane-mono. Spacing: Tailwind's default 4px scale already matches space-N (p-3 = 12px). */
  --font-pane: var(--font-sans);
  --font-pane-mono: var(--font-mono);
}
```

---

## 4. Global rules

### 4.1 Interaction states

| State | Bordered controls (button, pop-up, field) | Borderless controls (plain/toolbar button, sidebar row, menu item) |
|---|---|---|
| Rest | `control-bg` + `shadow-control` (fields: `field-bg` + 1px `control-border` + `shadow-field`) | transparent |
| Hover | **no change** (Mac behavior) | `fill` wash (sidebar rows: no change) |
| Pressed | `control-bg-pressed`; primary/destructive: `filter: brightness(0.9)` | `fill-strong` |
| Selected / on | segment: `segment-selected` + `shadow-control`; switch/check/radio: `accent` | sidebar: `fill-strong`; menu highlight: `accent-fill` + `on-accent`; table: `accent-fill` (focused) / `selection-inactive` (unfocused) |
| Disabled | whole control `opacity: var(--opacity-disabled)`, no hover or pressed change | label `label-tertiary` |
| Focus (keyboard) | `box-shadow: var(--shadow-control), var(--focus-ring)` | `box-shadow: var(--focus-ring)`; list rows: `inset 0 0 0 2px var(--accent)` |
| Invalid | 1px `red-text` outline + message with a warning icon | n/a |

### 4.2 Focus

- `focus-ring` is a solid 3px accent ring drawn with `box-shadow`, so it follows the border radius. It holds ≥3:1 on every surface in both themes.
- Show it on `:focus-visible` only. Never use `outline: none` without this replacement.
- For components whose focusable element is visually hidden (Checkbox, Radio), draw the ring on the visible part.

### 4.3 Motion

| Use | Duration | Easing | Notes |
|---|---|---|---|
| Switch knob and track | 200ms | `ease` | transform + background-color |
| Tooltip fade | 120ms | `ease` | fade only |
| Menu / Select popup open | 0ms | — | Mac menus appear instantly |
| Menu / Select popup close | 150ms | `ease-out` | opacity to 0 |
| Alert / dialog enter | 150ms | `ease-out` | opacity 0→1 and scale 0.96→1 from center |
| Progress value change | 200ms | `ease` | width |
| Indeterminate progress | 1.4s loop | `ease-in-out` | 35%-wide segment, translateX −100% → 290% |
| Spinner | 1s loop | `steps(12)` | rotate 360° |

Use Base UI's `[data-starting-style]` and `[data-ending-style]` (or Radix `[data-state]` with keyframes) for enter and exit. With `prefers-reduced-motion: reduce`, remove transitions, slow the spinner to 3s and the indeterminate bar to 4s, and drop scale effects.

### 4.4 Typography in components

UI text is 13px/16px (`body`). Small controls use 11px/14px. Section and group titles use `headline` (13px bold). Window titles are 15px/18px, weight 600. Secondary metadata uses `label-secondary`. Do not enlarge UI text for "readability". The 13px size is the native density.

### 4.5 Copy rules (enforce in demos and docs)

- **Title case:** buttons, menu items, window titles, toolbar labels, segment labels, table headers ("Move to Trash", "Date Modified").
- **Sentence case:** field labels, checkbox and switch labels, descriptions, help tags, messages ("Show hidden files").
- An ellipsis character `…` ends any command that needs more input ("Share…", "Sign Out…").
- Alert titles ask the question. Buttons answer with verbs, never Yes/No (*Delete “Archive”?* → Cancel · Delete).
- Address the person as "you". No exclamation marks, no emoji. Use curly quotes and apostrophes.
- Shortcuts use glyphs in the order ⌃ ⌥ ⇧ ⌘ ("⇧⌘Z").

### 4.6 Accessibility requirements

- Every control has an accessible name: a visible label, `aria-label` or `aria-labelledby`. Icon-only buttons need `aria-label` **and** a `Tooltip`.
- Composite widgets (SegmentedControl, Menu, Toolbar, Table, RadioGroup) are a single tab stop with arrow-key navigation, which the primitive provides.
- State is never conveyed by color alone: errors carry an icon and text, and the badge's meaning comes from its count or label.
- Hit areas below 24×24 (mini switch, small controls) are allowed in dense desktop UI. Offer the regular size in touch contexts.

---

## 5. Component specifications

Each spec covers purpose, what to build on, API, anatomy, exact visual values per size and state, keyboard behavior, accessibility and usage rules. Token names in `code` are CSS variables (`--name`).

---

### 5.1 Button

**Purpose.** The push button: one click, one command.

**Build on.** Base UI `Button` (`@base-ui/react/button`), or a native `<button type="button">`. Radix: native `<button>`. Inside a window toolbar, use Base UI `Toolbar.Button` (Radix `Toolbar.Button`) for roving focus.

**API**
```ts
interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'default' | 'primary' | 'destructive' | 'plain' | 'toolbar'; // default 'default'
  size?: 'small' | 'regular' | 'large';                                  // default 'regular'
  icon?: IconName;   // leading icon; with no children → icon-only (aria-label required)
}
```

**Anatomy.** `button[data-variant][data-size]` > optional `Icon` + `span.label`.

**Metrics**

| | small | regular | large | toolbar |
|---|---|---|---|---|
| Height | `control-small` 18px | `control-regular` 22px | `control-large` 28px | `control-large` 28px |
| Padding x | `space-2` 8px | `space-3` 12px | `space-4` 16px | `space-2` 8px |
| Icon-only width | 18px | 22px | 28px | 28px (min-width) |
| Font | 11/14 | 13/16 | 13/16 | 13/16 |
| Radius | 4px | `radius-sm` 5px | `radius-sm` | `radius-md` 6px |
| Icon size | 12 | 14 | 16 | 17 |

Icon-to-label gap: `space-1` (4px). `white-space: nowrap`, font-weight 400.

**Variants and states**

| Variant | Rest | Pressed | Text |
|---|---|---|---|
| default | `control-bg`, `shadow-control` | `control-bg-pressed` | `label` |
| primary | `accent-fill`, `shadow-control` | `brightness(0.9)` | `on-accent` |
| destructive | `danger-fill`, `shadow-control` | `brightness(0.9)` | `on-danger` |
| plain | transparent, no shadow; hover `fill` | `fill-strong` | `label` |
| toolbar | transparent; hover `fill`; `[aria-pressed=true]` → `fill-strong` | `fill-strong` | `label-secondary` (`label` when pressed) |

Disabled: `opacity: var(--opacity-disabled)`, with no hover or pressed change. Focus: `shadow-control, focus-ring` (plain/toolbar: `focus-ring`).

**Behavior.** In forms and dialogs, the primary button is `type="submit"` and activates on Return. Toggle-style toolbar buttons use `aria-pressed` (or Base UI `Toggle`).

**Usage.** Most buttons are `default`. Use at most one `primary` per window or dialog, placed last (trailing). Use `destructive` only inside a confirmation. Labels are verbs in title case.

---

### 5.2 SegmentedControl

**Purpose.** Switch between 2–5 mutually exclusive views or modes in place.

**Build on.** Base UI `ToggleGroup` (single, the default) with `Toggle` items (`@base-ui/react/toggle-group`, `@base-ui/react/toggle`). Radix `ToggleGroup` with `type="single"`. **Prevent an empty selection:** ignore `onValueChange` calls that would clear the value. (For a form value, a Base UI `RadioGroup` styled the same way is also acceptable.)

**API**
```ts
interface SegmentedControlProps {
  options: { value: string; label?: string; icon?: IconName; disabled?: boolean; 'aria-label'?: string }[];
  value?: string; defaultValue?: string; onValueChange?: (value: string) => void;
  size?: 'small' | 'regular' | 'large';
  'aria-label': string;
}
```

**Anatomy.** Track (`div[role=group]`) > segments (`button`, pressed state from the primitive) > optional `Icon` + label.

**Metrics**

| | small | regular | large |
|---|---|---|---|
| Track height | 18px | 22px | 28px |
| Track padding | 1px | 1px | 1px |
| Segment padding x | 8px | 12px | 16px |
| Segment radius | 3px | 4px | 4px |
| Font | 11/14 | 13/16 | 13/16 |
| Icon | 12 | 14 | 14 |

- Track: `fill-strong` background, `radius-sm`.
- Divider: a 1px `separator` line between adjacent unselected segments, inset 4px top and bottom. Hide it on both sides of the selected segment.
- Selected segment: `segment-selected` with `shadow-control`, raised above its neighbors (`z-index: 1`).
- Text: `label`; disabled segments use `label-tertiary`.
- Focus: `focus-ring` on the segment (`z-index: 2`).

**Keyboard.** One tab stop. Arrow keys move focus (roving, from the primitive). Space or Enter selects.

**Usage.** Use title-case nouns ("Day · Week · Month"). Don't mix icon-only and text segments in one control. For persistent settings, use `Select` or `RadioGroup` instead.

---

### 5.3 TextField

**Purpose.** Single-line text input with label, helper text and error state.

**Build on.** Base UI `Field.Root`, `Field.Label`, `Field.Control` (or `Input`), `Field.Description`, `Field.Error` (`@base-ui/react/field`). Radix: `Label` plus a native `<input>`.

**API**
```ts
interface TextFieldProps extends Omit<React.InputHTMLAttributes<HTMLInputElement>, 'size'> {
  label?: React.ReactNode;      // omit inside FormRow (the row labels it)
  description?: React.ReactNode; // helper text
  error?: React.ReactNode;       // when present → invalid
  size?: 'small' | 'regular' | 'large';
}
```

**Metrics**

| | small | regular | large |
|---|---|---|---|
| Height | 18px | 22px | 28px |
| Padding x | 5px | 6px | 8px |
| Font | 11/14 | 13/16 | 13/16 |
| Radius | 4px | `radius-sm` | `radius-sm` |

- Root: vertical stack with a `space-1` gap. Label is 13/16 `label`.
- Input: `field-bg`; `box-shadow: inset 0 0 0 1px var(--control-border), var(--shadow-field)`; text `label`; placeholder `label-secondary` at opacity 1.
- Focus: `box-shadow: inset 0 0 0 1px var(--accent), var(--focus-ring)`.
- Invalid (`[data-invalid]` or `aria-invalid`): the inset outline becomes `red-text`. The message renders in `red-text` with a 12px `warning` icon.
- Description: 11/14 `label-secondary`.
- Disabled: `opacity-disabled`.

**Usage.** Placeholders are examples, never labels. Error text says how to fix the problem ("Enter a number up to 65535."). Size the width to the expected content.

---

### 5.4 SearchField

**Purpose.** Rounded search input with a magnifier, a clear button and Escape-to-clear.

**Build on.** Base UI `Input` (optionally within `Field`), rendered as `type="search"`. Radix: native `<input type="search">`.

**API.** `TextFieldProps` minus `label` and `error`, plus `onClear?: () => void`. Default placeholder: "Search". Default accessible name: the placeholder.

**Metrics**

| | small | regular | large |
|---|---|---|---|
| Height | 18px | 22px | 28px |
| Radius | 4px | `radius-md` 6px | `radius-md` |
| Padding-left | 22px | 25px | 28px |
| Magnifier | 11px at left 6px | 13px at left 7px | 15px at left 8px |
| Padding-right | 24px | 24px | 24px |

- Input styling is the same as TextField.
- Magnifier: `search` icon in `label-secondary`, `pointer-events: none`.
- Clear button: 16×16 at right 4px, `xmark-circle` icon 14px in `label-secondary` (hover `label`), `tabIndex={-1}`, shown only when the field has a value. After clearing, return focus to the input.
- Hide the native cancel button: `::-webkit-search-cancel-button { appearance: none; }`.

**Behavior.** Escape clears a non-empty field and calls `onClear`. Filter live as the person types, with no submit button. In a window toolbar, use `large` as the last toolbar item.

---

### 5.5 Checkbox

**Purpose.** An on/off choice that takes effect when a form is saved.

**Build on.** Base UI `Checkbox.Root` + `Checkbox.Indicator` (`@base-ui/react/checkbox`; state attributes `data-checked`, `data-unchecked`, `data-indeterminate`, `data-disabled`). Use Base UI `CheckboxGroup` for parent/child selection. Radix `Checkbox.Root` + `Checkbox.Indicator` (`data-state`).

**API.** `checked`, `defaultChecked`, `onCheckedChange(checked)`, `indeterminate`, `disabled`, `label`.

**Metrics and states**

- Box: 14×14, `radius-xs` (3px), `field-bg`, `box-shadow: inset 0 0 0 1px var(--control-border), var(--shadow-field)`.
- Checked or indeterminate: `accent` background, no outline. The glyph is drawn in `on-accent` in a 12×12 viewBox with stroke 1.9 and round caps and joins:
  - check: `M2.6 6.3 L5 8.6 L9.4 3.4`
  - dash: `M3 6 H9`
- Label: 13/16 `label`, gap 6px to the right of the box. The whole label is the click target.
- Focus: `focus-ring` on the box (plus the outline when unchecked).
- Disabled: label `label-tertiary`, box at `opacity-disabled`.

**Usage.** Labels describe the on state in sentence case. If a setting applies immediately, use `Switch`. For exclusive choices, use `Radio`.

---

### 5.6 Radio (RadioGroup)

**Purpose.** One choice from 2–5 visible, mutually exclusive options.

**Build on.** Base UI `RadioGroup` + `Radio.Root` + `Radio.Indicator` (`@base-ui/react/radio-group`, `@base-ui/react/radio`), wrapped in `Fieldset.Root` + `Fieldset.Legend`. Radix `RadioGroup.Root` / `Item` / `Indicator`.

**Metrics and states**

- Ring: 16×16, `radius-full`, `field-bg`, with the same inset outline and `shadow-field` as Checkbox.
- Checked: `accent` fill, no outline, and a 6×6 `on-accent` dot centered.
- Group: vertical, `space-2` (8px) gap. Legend is 13px `label` with 8px below and ends with a colon ("Appearance:").
- Focus, disabled and label rules are the same as Checkbox.

**Keyboard.** One tab stop. Arrow keys move and select (from the primitive).

**Usage.** Always have one option selected by default. With more than 5 options, use `Select`.

---

### 5.7 Switch

**Purpose.** A setting that applies immediately.

**Build on.** Base UI `Switch.Root` + `Switch.Thumb` (`@base-ui/react/switch`; `data-checked` / `data-unchecked`). Radix `Switch.Root` + `Switch.Thumb`.

**API.** `checked`, `defaultChecked`, `onCheckedChange(checked)`, `size: 'mini' | 'small' | 'regular'` (default `regular`), `label?`.

**Metrics**

| | mini | small | regular |
|---|---|---|---|
| Track | 26×15 | 32×18 | 38×22 |
| Thumb | 13px | 16px | 20px |
| Thumb travel (checked) | 11px | 14px | 16px |

- Thumb inset 1px top and left. `radius-full` throughout.
- Off: track `fill-strong` with `inset 0 0 0 1px var(--control-border)`, so the off state stays visible at 3:1.
- On: track `accent`, no outline.
- Thumb: `knob` background with `shadow-knob`.
- Transition: 200ms ease on transform and background-color.
- Focus: `focus-ring` (plus the outline when off). Disabled: `opacity-disabled`.
- With a label: label to the right, gap `space-2`, 13px `label`.

**Usage.** In settings rows (FormRow), use `small`, placed at the trailing edge. Don't use a switch in a form that has a Save button.

---

### 5.8 Select (pop-up button)

**Purpose.** A push button that opens a list of exclusive choices. It is recognizable by the accent "chevron well".

**Build on.** Base UI `Select` (`@base-ui/react/select`: `Root`, `Trigger`, `Value`, `Icon`, `Portal`, `Positioner`, `Popup`, `List`, `Item`, `ItemText`, `ItemIndicator`, `Separator`, `Group`, `GroupLabel`). Keep `Select.Positioner alignItemWithTrigger` at its default `true`: it overlaps the popup on the trigger like macOS. Radix `Select` with `position="item-aligned"`. A styled native `<select>` is an acceptable fallback (the Pane prototype uses one).

**API.** `value`, `defaultValue`, `onValueChange`, `items: {value, label, disabled?}[]` (or children), `size`, `disabled`, `placeholder`.

**Trigger metrics**

| | small | regular | large |
|---|---|---|---|
| Height | 18px | 22px | 28px |
| Padding-left | 6px | 8px | 12px |
| Padding-right | 22px | 28px | 32px |
| Font | 11/14 | 13/16 | 13/16 |
| Radius | 4px | `radius-sm` | `radius-sm` |
| Chevron well | 13px wide, inset 2px | 16px wide, inset 3px | 20px wide, inset 4px |
| Chevron icon | 9px | 11px | 11px |

- Trigger: same as the default Button (`control-bg`, `shadow-control`, `label`). Pressed or open: `control-bg-pressed`.
- Chevron well: absolutely positioned at the trailing edge, `radius-xs`, `accent-fill` background, `chevron-up-down` icon in `on-accent` at stroke 2.
- Popup: styled exactly like Menu (§5.16). The selected item shows an 11px checkmark in the leading 14px column. The highlighted item is `accent-fill` with `on-accent` text.
- Focus: `shadow-control, focus-ring`. Disabled: `opacity-disabled`.

**Usage.** Use when there are more than 5 options or space is tight. Labels are title case. Size the trigger to the longest option.

---

### 5.9 Slider

**Purpose.** Pick a value from a continuous range.

**Build on.** Base UI `Slider.Root`, `Slider.Control`, `Slider.Track`, `Slider.Indicator`, `Slider.Thumb` (`@base-ui/react/slider`; `data-dragging`, `data-disabled`). Radix `Slider.Root`, `Track`, `Range`, `Thumb`.

**API.** `value`, `defaultValue`, `onValueChange(number)`, `min` (0), `max` (100), `step` (1), `size: 'small' | 'regular'`, `aria-label`.

**Metrics**

| | small | regular |
|---|---|---|
| Control height | 16px | 20px |
| Track | 4px, `radius-full` | 4px, `radius-full` |
| Thumb | 15px circle | 20px circle |

- Track: `fill-strong`. Indicator (filled part): `accent`.
- Thumb: `knob` with `shadow-knob`. Focus: `shadow-knob, focus-ring` on the thumb.
- Disabled: `opacity-disabled`.
- Default width is 200px. The consumer may set any width.

**Usage.** Show what the ends mean with small icons at each side (− / +). When the exact value matters, show it next to the slider.

---

### 5.10 ProgressBar

**Purpose.** Show progress of a task with a known length, or an indeterminate wait.

**Build on.** Base UI `Progress.Root`, `Progress.Track`, `Progress.Indicator`, `Progress.Label` (`@base-ui/react/progress`; `value={null}` means indeterminate). Use Base UI `Meter` for static quantities such as storage. Radix `Progress.Root` + `Progress.Indicator`.

**API.** `value?: number | null` (0–1 in the kit API; convert to the primitive's scale), `size: 'small' | 'regular'`, plus a label via `aria-labelledby` or `Progress.Label`.

**Metrics.** Height 6px (small 4px), `radius-full`, track `fill-strong`, indicator `accent`, width transition 200ms. Indeterminate: indicator 35% wide, animated as in §4.3. The width is the consumer's (default 200px).

**Usage.** Always pair with visible text describing the task ("Copying 3 of 12 items"). Switch to determinate as soon as the total is known. Progress never moves backwards.

---

### 5.11 Spinner

**Purpose.** The twelve-spoke activity indicator for short waits.

**Build on.** Custom component (no primitive).

**Spec.** An SVG with a 16×16 viewBox containing 12 lines radiating from (8,8) from radius 3.6 to 6.6 at 30° steps, starting at the top. Stroke is `currentColor` with width 1.6 (1.3 when rendered larger than 24px) and round caps. Spoke *i* (0–11) has opacity `0.18 + 0.82 × i/11`. The whole SVG rotates 360° in 1s with `steps(12)`. Color is `label-secondary`. Sizes are 16px inline, 32px for loading a view, and 12px inside a button. Use `role="progressbar"` with `aria-label` (default "Loading"). Reduced motion: 3s per turn.

**Usage.** For waits under a few seconds. For anything longer, use ProgressBar with a description. A busy button is also disabled.

---

### 5.12 Badge

**Purpose.** A count or one short status word beside a sidebar row, tab or label.

**Build on.** Custom `span`.

**Spec.** Height 16px, min-width 18px, padding 0 6px, `radius-full`, 11/14 weight 500, `tabular-nums`, `white-space: nowrap`.

| tone | background | text | use |
|---|---|---|---|
| neutral (default) | `fill-strong` | `label` | unread counts |
| accent | `accent-fill` | `on-accent` | something new |
| danger | `danger-fill` | `on-danger` | needs attention |

**Usage.** Abbreviate large numbers ("1.2K"). No sentences.

---

### 5.13 Tooltip (help tag)

**Purpose.** A short description shown after a pause on hover or focus.

**Build on.** Base UI `Tooltip.Provider`, `Root`, `Trigger`, `Portal`, `Positioner`, `Popup` (`@base-ui/react/tooltip`). Set `delay={500}` on the Provider (Base UI's trigger default is 600ms) and `Positioner side="bottom" align="start" sideOffset={6}`. Radix `Tooltip.Provider delayDuration={500}`, `Content side="bottom" align="start" sideOffset={6}`.

**Spec.** `menu-bg` material (§2.5), `radius-md`, `shadow-menu`, padding 3px 7px, 12/15 `label`, max-width 280px, no arrow. Fade 120ms.

**Usage.** Required on every icon-only button. Describe what the control does ("Share this document"). No interactive content inside. Closes on Escape, blur and pointer leave.

---

### 5.14 TrafficLights

**Purpose.** A window's close, minimize and zoom controls.

**Build on.** Custom (three `<button>`s).

**Spec**

- 3 circles of 12×12, gap 8px, `radius-full`, rim `inset 0 0 0 0.5px var(--window-control-rim)`.
- Colors: `window-close`, `window-minimize`, `window-zoom`.
- Glyphs (12×12 viewBox, stroke `window-control-glyph` 1.2px, round caps). They are hidden until the pointer is over the **group**, or a light has `:focus-visible`:
  - close `M3.75 3.75 L8.25 8.25 M8.25 3.75 L3.75 8.25`
  - minimize `M3.25 6 H8.75`
  - zoom (filled with `window-control-glyph`, no stroke) `M3.4 7.9 V3.4 H7.9 Z M8.6 4.1 V8.6 H4.1 Z`
- Inactive window: all three lights use `window-control-inactive`. Hovering the group restores their colors.
- A disabled light (e.g. zoom on a fixed-size panel) uses `window-control-inactive` and shows no glyph.
- `aria-label`: "Close", "Minimize", "Zoom". They are `tabIndex={-1}` by default, matching macOS. Expose a `focusable` prop to opt in.

---

### 5.15 Window

**Purpose.** The macOS window frame: unified title bar and toolbar, optional translucent sidebar, and content area.

**Build on.** Custom layout. Use Base UI `Toolbar.Root` (Radix `Toolbar.Root`) for the toolbar items to get roving focus, and Base UI `ScrollArea` for overlay scrollbars if wanted. If the window is shown modally, wrap it in `Dialog`.

**API**
```ts
interface WindowProps {
  title?: React.ReactNode; subtitle?: React.ReactNode;
  toolbar?: React.ReactNode;           // trailing toolbar items
  sidebar?: React.ReactNode;           // usually <Sidebar>
  sidebarAccessory?: React.ReactNode;  // e.g. a sidebar-toggle toolbar button
  sidebarWidth?: number | string;      // default var(--sidebar-width) 220px
  toolbarStyle?: 'unified' | 'compact';// default 'unified'
  active?: boolean;                    // default true
  onClose?(): void; onMinimize?(): void; onZoom?(): void;
  width?: number | string; height?: number | string;
  children?: React.ReactNode;
}
```

**Anatomy**
```
div.window (role=group, aria-label=title)
├─ aside.sidebar                 (only with `sidebar`)
│  ├─ div.sidebar-top            TrafficLights · [sidebarAccessory → trailing]
│  └─ div.sidebar-body           {sidebar}
└─ div.main
   ├─ header.toolbar             [TrafficLights if no sidebar] · titles · tools(→ trailing)
   └─ div.content                {children}
```

**Metrics**

- Window: `radius-lg`, `overflow: hidden`, `content-bg`, `shadow-window`, min-height 160px, 13/16 text.
- Sidebar: width `--sidebar-width` (220px), `sidebar-bg` material, right hairline `box-shadow: inset -1px 0 0 var(--separator)`. Top area height `toolbar-height` (52px), padding 0 12px 0 20px (`space-5` before the lights), gap 8px, accessory pushed to the trailing edge. Body scrolls, with 12px bottom padding.
- Toolbar (unified): height `toolbar-height` 52px, padding 0 12px 0 20px, gap 12px, `toolbar-bg` material, bottom hairline `inset 0 -1px 0 var(--separator)`. Without a sidebar, the lights come first, followed by an extra 8px gap.
- Title: 15/18 weight 600, single line with ellipsis. Subtitle: 11/14 `label-secondary`.
- Tools: `margin-left: auto`, gap 4px. Order: navigation buttons, flexible space, view controls, actions, SearchField last.
- Compact: height `titlebar-height` 28px, padding 0 8px. The title is 13/16 weight 600, centered absolutely between 70px insets. No tools.
- Content: `content-bg`, `flex: 1`, scrolls.
- Inactive: title `label-secondary`, materials drop the blur, lights inactive.

**Usage.** On the web this is a container, not an OS window. Don't simulate dragging or resizing unless you actually implement them.

---

### 5.16 Menu

**Purpose.** A command menu (pull-down, context or menubar).

**Build on.** Base UI `Menu` (`@base-ui/react/menu`: `Root`, `Trigger`, `Portal`, `Positioner`, `Popup`, `Item`, `LinkItem`, `Separator`, `Group`, `GroupLabel`, `CheckboxItem`, `CheckboxItemIndicator`, `RadioGroup`, `RadioItem`, `RadioItemIndicator`, `SubmenuRoot`, `SubmenuTrigger`). Base UI `ContextMenu` and `Menubar` use the same item styling. Radix `DropdownMenu`, `ContextMenu`, `Menubar`.

**API (kit).** Export styled parts that mirror the primitive (`Menu.Root`, `Menu.Trigger`, `Menu.Popup`, `Menu.Item`, `Menu.CheckboxItem`, `Menu.RadioItem`, `Menu.Separator`, `Menu.GroupLabel`, `Menu.SubmenuTrigger`). `Menu.Item` additionally takes `shortcut?: string` and `icon?: IconName`.

**Popup metrics**

- Min-width 200px, padding 5px, `menu-bg` material, `radius-md`, `shadow-menu`, 13/16 `label`, `user-select: none`.
- Item: height 22px, padding 0 8px, `radius` 4px, gap 6px, `display: flex; align-items: center`.
- When any item in the menu is checkable, reserve a 14px leading check column on every item and reduce the left padding to 4px. The checkmark icon is 11px.
- Icon: 14px, `label-secondary`.
- Label: `flex: 1`, nowrap.
- Shortcut: `label-secondary`, margin-left 24px, letter-spacing 0.04em.
- Submenu chevron: `chevron-right` 11px, `label-secondary`, margin-left 16px.
- Highlighted (`[data-highlighted]`): `accent-fill` background, with `on-accent` for the label, shortcut, icon and chevron.
- Disabled: `label-tertiary` for the label, shortcut and icon. The item still appears.
- Separator: 1px `separator`, margin 5px 8px.
- Group label: 11/14 weight 600 `label-secondary`, padding 3px 8px 1px.
- Positioning: below the trigger, aligned to its start, offset 4px. Submenus open to the side with offset −5px so their items align.

**Behavior.** Opens instantly and fades out over 150ms (§4.3). Keyboard support (arrows, typeahead, Enter, Escape, submenus) comes from the primitive.

**Usage.** Title-case labels, with an ellipsis for commands needing more input. Group related commands with separators. Disable unavailable commands rather than hiding them.

---

### 5.17 Sidebar

**Purpose.** A source list: grouped navigation rows at a window's leading edge.

**Build on.** Custom: `nav` > section > `ul` > `li` > `button` (or `a`). Use Base UI `Collapsible` (Radix `Collapsible`) for collapsible sections.

**API**
```ts
interface SidebarProps {
  sections: { title?: React.ReactNode; items: { id: string; label: React.ReactNode; icon?: IconName; badge?: React.ReactNode; badgeTone?: 'neutral'|'accent'|'danger'; disabled?: boolean }[] }[];
  value?: string; defaultValue?: string; onValueChange?: (id: string) => void;
  'aria-label'?: string; // default "Sidebar"
}
```

**Metrics**

- `nav` padding 0 8px. Sections are stacked with a 12px gap.
- Section heading: 11px / 20px line-height, weight 600, `label-secondary`, padding 0 8px, 2px below.
- Row: height `sidebar-row` (28px), padding 0 8px, gap 7px, `radius-md`, 13/16 `label`, left-aligned, label ellipsized.
- Row icon: 16px in `accent`. A badge sits at the trailing edge.
- Selected (`aria-current="page"`): `fill-strong` background.
- Hover: no change (Mac behavior).
- Focus: `box-shadow: inset 0 0 0 2px var(--accent)`.
- Disabled: `label-tertiary`, icon at `opacity-disabled`.

**Usage.** Place it in `Window`'s sidebar or on any `sidebar-bg` surface. Labels are one or two words, counts go in a `Badge`, and section titles are short ("Favorites", "Locations").

---

### 5.18 Alert

**Purpose.** A small centered dialog that confirms or warns before something consequential.

**Build on.** Base UI `AlertDialog` (`@base-ui/react/alert-dialog`: `Root`, `Trigger`, `Portal`, `Backdrop`, `Popup`, `Title`, `Description`, `Close`). Radix `AlertDialog` (`Overlay`, `Content`, `Title`, `Description`, `Cancel`, `Action`).

**API (kit)**
```ts
interface AlertProps {
  open?: boolean; defaultOpen?: boolean; onOpenChange?(open: boolean): void;
  icon?: React.ReactNode;       // 64px app icon / symbol
  title: React.ReactNode;       // the question
  message?: React.ReactNode;    // consequence; whether it can be undone
  actions: { label: string; variant?: 'default'|'primary'|'destructive'; onClick?(): void }[]; // visual order, default last
  children?: React.ReactNode;   // e.g. a “Don’t ask again” Checkbox
}
```

**Metrics**

- Popup: width 260px, padding 20px 16px 16px, `window-bg`, `radius-lg`, `shadow-window`, centered column, `text-align: center`, centered in the viewport.
- Icon: 64×64, 12px below.
- Title: 13/16 bold `label`. Message: 11/14 `label`, 8px above.
- Extra content: 12px above, centered.
- Actions: 16px above. Two actions sit side by side at equal width (`flex: 1`) with an 8px gap. Three or more stack vertically with the **default on top** (`flex-direction: column-reverse` over the visual-order list).
- Backdrop: transparent by default, to catch outside clicks. For a dimmed variant, use `fill-strong`.
- Motion: 150ms ease-out fade with scale 0.96 → 1 (§4.3).

**Behavior.** Return triggers the default (primary) action and Escape triggers Cancel. Initial focus goes to the default button, except when the default is destructive: then focus Cancel.

**Usage.** Name buttons with verbs that answer the title. Never use "Yes/No". Cancel must always be the safe choice.

---

### 5.19 GroupBox

**Purpose.** A rounded settings group in the System Settings style.

**Build on.** Base UI `Fieldset.Root` + `Fieldset.Legend` when the group holds form controls; otherwise `section` + `h3`. Radix: `section` + heading.

**Spec.** A vertical stack with a 6px gap.

- Title: 13/16 bold `label`, 8px start margin.
- Box: `group-bg`, `radius-lg`, `box-shadow: inset 0 0 0 0.5px var(--separator)`.
- Footer: 11/14 `label-secondary`, 8px side margins.

Place groups on `content-bg`, stacked 24px (`space-6`) apart.

---

### 5.20 FormRow

**Purpose.** One setting inside a GroupBox: label on the leading side, control on the trailing side.

**Build on.** Base UI `Field.Root` + `Field.Label` + `Field.Description` (associates the label and description with the control). Radix `Label` + `aria-describedby`.

**API.** `label`, `description?`, `icon?: IconName`, `iconColor?: 'blue'|'purple'|'red'|'pink'|'accent-fill'` (default `blue`), `children` (the control).

**Spec**

- Row: min-height 40px, padding 8px 12px, gap 12px, `align-items: center`.
- Between rows: a 1px `separator` line inset 12px from both ends (none above the first row).
- Icon tile: 22×22, `radius-sm`, background is the chosen color token, glyph 14px in `on-accent`. Only the five listed colors keep a white glyph at ≥3:1 in both themes.
- Text: label 13/16 `label`; description 11/14 `label-secondary`, 2px gap.
- Control: trailing, gap 8px between multiple controls. Switches use `small`.
- Navigation rows end with a 12px `chevron-right` icon in `label-secondary` instead of a control.

---

### 5.21 Table

**Purpose.** A multi-column list with zebra stripes and single-row selection.

**Build on.** Custom: a native `<table>` with `role="grid"` and `aria-selected` on rows. Use TanStack Table or Virtual for sorting and virtualization logic if needed. Base UI and Radix have no table primitive.

**API.** `columns: {key, title, width?, align?: 'left'|'right'|'center', secondary?, render?(row)}[]`, `rows`, `rowKey = 'id'`, `value`, `defaultValue`, `onValueChange`, `striped = true`, `aria-label`.

**Metrics**

- Wrapper: `content-bg`, padding 0 8px 8px. The table uses `border-collapse: separate` with 0 spacing.
- Header cell: height 28px, padding 0 8px, 11px weight 500 `label-secondary`, background `content-bg`, bottom hairline `inset 0 -1px 0 var(--separator)`. Between header cells, a 1px `separator` line inset 7px top and bottom.
- Body cell: height `row-height` (24px), padding 0 8px, 13/16 `label`, nowrap with ellipsis. Secondary columns use `label-secondary`. `tabular-nums`. Right-align numbers.
- Zebra: even rows `content-bg-alt`.
- Selection: rounded ends (`radius-sm` on the first and last cells). `selection-inactive` when the table is not focused; `accent-fill` + `on-accent` when it is.
- Focus: `box-shadow: inset 0 0 0 2px var(--accent)` on `:focus-visible`.

**Keyboard.** The table is one tab stop. Up and Down move the selection (Home and End are optional). Enter is the consumer's "open" action.

---

### 5.22 Icon

**Purpose.** Render Pane's 32 line icons.

**Build on.** Custom component using the data in Appendix B.

**Contract.** A 16×16 viewBox, `fill="none"`, `stroke="currentColor"`, `stroke-width="1.5"`, `stroke-linecap="round"`, `stroke-linejoin="round"`. Elements marked `fill` render with `fill: currentColor; stroke: none`. Elements marked `knockout` render with `stroke: var(--field-bg)` (used by `xmark-circle`). Props: `name`, `size` (default 16), and `title`, which switches the icon from `aria-hidden="true"` to `role="img"` with a `<title>`. Icons take the color of their context: `accent` in sidebars, `label-secondary` in toolbars and menus, `label` in content.

---

## 6. Composition recipes

### 6.1 Finder-style window

```tsx
<Window
  title="Documents" subtitle="6 items" height={380}
  sidebarAccessory={<Button variant="toolbar" icon="sidebar" aria-label="Hide Sidebar" />}
  sidebar={
    <Sidebar aria-label="Locations" defaultValue="docs" sections={[
      { title: 'Favorites', items: [
        { id: 'recents', label: 'Recents', icon: 'clock' },
        { id: 'docs', label: 'Documents', icon: 'doc' },
        { id: 'downloads', label: 'Downloads', icon: 'download', badge: 3 } ] },
      { title: 'Locations', items: [{ id: 'cloud', label: 'Cloud Drive', icon: 'cloud' }] } ]} />
  }
  toolbar={<>
    <Button variant="toolbar" icon="chevron-left" aria-label="Back" />
    <Button variant="toolbar" icon="chevron-right" aria-label="Forward" disabled />
    <span style={{ flex: 1 }} />
    <SegmentedControl aria-label="View" defaultValue="list" options={[
      { value: 'grid', icon: 'grid', 'aria-label': 'Icons' }, { value: 'list', icon: 'list', 'aria-label': 'List' }]} />
    <Button variant="toolbar" icon="share" aria-label="Share" />
    <SearchField size="large" style={{ width: 160 }} />
  </>}
>
  <Table aria-label="Files" rows={files} columns={[
    { key: 'name', title: 'Name' },
    { key: 'date', title: 'Date Modified', width: 190, secondary: true },
    { key: 'size', title: 'Size', width: 70, align: 'right', secondary: true },
    { key: 'kind', title: 'Kind', width: 160, secondary: true } ]} />
</Window>
```

### 6.2 Settings pane

```tsx
<div style={{ background: 'var(--content-bg)', padding: 'var(--space-5)', display: 'grid', gap: 'var(--space-6)', maxWidth: 520 }}>
  <GroupBox title="Notifications" footer="Previews show the first lines of a message.">
    <FormRow label="Allow notifications" icon="bell" iconColor="red"><Switch size="small" defaultChecked /></FormRow>
    <FormRow label="Show previews">
      <Select defaultValue="unlocked" items={[{ value: 'always', label: 'Always' }, { value: 'unlocked', label: 'When Unlocked' }, { value: 'never', label: 'Never' }]} />
    </FormRow>
    <FormRow label="Play sound for notifications" description="Uses the alert sound set in Sound settings."><Switch size="small" /></FormRow>
  </GroupBox>
</div>
```

### 6.3 Destructive confirmation

```tsx
<Alert
  icon={<AppIcon size={64} />}
  title="Delete “Archive”?"
  message="This item will be deleted immediately. You can’t undo this action."
  actions={[{ label: 'Cancel' }, { label: 'Delete', variant: 'destructive', onClick: remove }]}
>
  <Checkbox label="Don’t ask again" />
</Alert>
```
Initial focus goes to **Cancel**, because the default action is destructive.

### 6.4 Context menu on a row

Wrap the row in Base UI `ContextMenu.Root` / `ContextMenu.Trigger` and reuse the Menu popup and item styles from §5.16. Items: "Open", "Share…", "Add Tag", a separator, then "Move to Trash" with shortcut "⌘⌫".

---

## 7. Extending Pane beyond the 22 components

Derive new components from the nearest spec and the tokens. Do not introduce new visual language.

| Component | Build on (Base UI / Radix) | Derive from |
|---|---|---|
| Popover | `Popover` / `Popover` | Menu popup surface (`menu-bg` material, `radius-md`, `shadow-menu`), padding `space-4`, optional 8px arrow in `menu-bg` |
| Dialog / Sheet | `Dialog` / `Dialog` | Alert surface; sheets attach under the toolbar, are as wide as content, `radius-lg` on the bottom corners only |
| Context menu, Menubar | `ContextMenu`, `Menubar` / same | Menu (§5.16) exactly |
| Tabs | `Tabs` / `Tabs` | Prefer SegmentedControl styling, centered above the content |
| Combobox / Autocomplete | `Combobox`, `Autocomplete` / — | TextField input with a Menu-styled list |
| Number field (stepper) | `NumberField` / — | TextField plus a 13×22 stepper of two stacked `control-bg` halves with `chevron` glyphs |
| Scroll area | `ScrollArea` / `ScrollArea` | Overlay scrollbars: 7px thumb, `radius-full`, `label` at 35% opacity, shown while scrolling |
| Disclosure | `Collapsible` / `Collapsible` | `chevron-right` 11px rotating 90° when open, 13px `label` |
| Toast / notification | `Toast` / `Toast` | `menu-bg` material, `radius-lg`, `shadow-menu`, 13px bold title + 11px message, top-right, 340px wide |
| Toggle (single) | `Toggle` / `Toggle` | Toolbar button with `aria-pressed` styling |
| Avatar | `Avatar` / `Avatar` | Circle, `gray` fill, initials in `on-accent` 11–13px weight 600 |

Anything else: ask before inventing.

---

## 8. Acceptance checklist

**Tokens and theming**
- [ ] `tokens.css` is Appendix A verbatim, and no component file contains a raw color.
- [ ] Light by default. `data-theme="dark"` switches everything, including content in portals. No attribute follows the OS setting.
- [ ] `color-scheme` follows the theme (native scrollbars and select lists match).

**Every component**
- [ ] Every size, variant and state in §5 is visible in a demo, in both themes.
- [ ] Metrics match the spec to the pixel (check with devtools: heights, paddings, radii, font sizes).
- [ ] Keyboard alone can reach and operate it, and the focus ring appears on `:focus-visible` only.
- [ ] It has an accessible name, and icon-only buttons have a tooltip.
- [ ] Disabled styling uses `opacity-disabled` or `label-tertiary` and shows no hover or pressed feedback.
- [ ] Controls use `cursor: default`.

**Contrast (automated)**
- [ ] Every pair in §3.8 meets its minimum in both themes (composite translucent colors before measuring).

**Behavior**
- [ ] The primary button submits on Return. Alerts focus the safe button when the default is destructive.
- [ ] SegmentedControl never ends up with no selection.
- [ ] Select's popup overlaps the trigger with the selected item aligned (where the primitive supports it).
- [ ] Menus open instantly, show shortcuts, and keep disabled items visible.
- [ ] With reduced motion: no transitions, slower spinner and indeterminate bar, no scale.

**Copy**
- [ ] Demos follow §4.5 (title case commands, ellipses, verbs on alert buttons, curly quotes).

---

## Appendix A — `tokens.css`

Copy verbatim. Generated from the Pane design system's `tokens.json`.

```css
/* Pane — design tokens. Generated from tokens.json (v1). Light is the default theme.
   Theme: <html data-theme="light|dark">; no attribute = follow the OS (prefers-color-scheme). */

:root,
[data-theme="light"] {
  /* color */
  --window-bg: #ececec;
  --content-bg: #ffffff;
  --content-bg-alt: #f4f5f5;
  --toolbar-bg: #f6f6f6eb;
  --sidebar-bg: #e8e8e8d9;
  --menu-bg: #f2f2f2eb;
  --control-bg: #ffffff;
  --control-bg-pressed: #e3e3e3;
  --field-bg: #ffffff;
  --fill: #0000000d;
  --fill-strong: #0000001a;
  --group-bg: #0000000a;
  --knob: #ffffff;
  --selection-inactive: #dcdcdc;
  --segment-selected: var(--control-bg);
  --separator: #0000001a;
  --control-border: #808085;
  --label: #000000d9;
  --label-secondary: #00000099;
  --label-tertiary: #00000047;
  --accent: var(--blue);
  --accent-fill: #0063e1;
  --on-accent: #ffffff;
  --link: #0066cc;
  --red: #ff3b30;
  --orange: #ff9500;
  --yellow: #ffcc00;
  --green: #28cd41;
  --blue: #007aff;
  --purple: #af52de;
  --pink: #ff2d55;
  --gray: #8e8e93;
  --red-text: #d70015;
  --orange-text: #b83000;
  --green-text: #1a7431;
  --danger-fill: #d70015;
  --on-danger: #ffffff;
  --window-close: #ff5f57;
  --window-minimize: #febc2e;
  --window-zoom: #28c840;
  --window-control-inactive: #d1d1d1;
  --window-control-rim: #00000026;
  --window-control-glyph: #0000008c;
  /* shadow */
  --shadow-control: 0 0 0 0.5px #0000001f, 0 0.5px 1.5px #00000033;
  --shadow-field: inset 0 0.5px 1px #0000001a;
  --shadow-knob: 0 0 0 0.5px #00000029, 0 1px 2.5px #00000033;
  --shadow-menu: 0 0 0 0.5px #0000002e, 0 10px 32px #00000038;
  --shadow-window: 0 0 0 0.5px #00000040, 0 4px 12px #0000001f, 0 22px 56px #00000047;
  --focus-ring: 0 0 0 3px #007aff;
}

[data-theme="dark"] {
  /* color */
  --window-bg: #323232;
  --content-bg: #1e1e1e;
  --content-bg-alt: #282828;
  --toolbar-bg: #363636eb;
  --sidebar-bg: #282828d9;
  --menu-bg: #2c2c2ceb;
  --control-bg: #636366;
  --control-bg-pressed: #4f4f51;
  --field-bg: #2c2c2e;
  --fill: #ffffff12;
  --fill-strong: #ffffff1f;
  --group-bg: #ffffff0d;
  --knob: #e5e5e7;
  --selection-inactive: #3a3a3a;
  --segment-selected: var(--control-bg);
  --separator: #ffffff1a;
  --control-border: #8e8e93;
  --label: #ffffffd9;
  --label-secondary: #ffffff8c;
  --label-tertiary: #ffffff40;
  --accent: var(--blue);
  --accent-fill: #0058d0;
  --link: #4aa3ff;
  --red: #ff453a;
  --orange: #ff9f0a;
  --yellow: #ffd60a;
  --green: #32d74b;
  --blue: #0a84ff;
  --purple: #bf5af2;
  --pink: #ff375f;
  --gray: #98989d;
  --red-text: #ff6961;
  --orange-text: #ffb340;
  --green-text: #30db5b;
  --danger-fill: #d92418;
  --window-control-inactive: #4e4e4e;
  --window-control-rim: #0000004d;
  /* shadow */
  --shadow-control: inset 0 0.5px 0 #ffffff26, 0 0 0 0.5px #00000066, 0 0.5px 1.5px #00000066;
  --shadow-field: inset 0 0.5px 1px #00000040;
  --shadow-knob: 0 0 0 0.5px #00000066, 0 1px 2.5px #00000080;
  --shadow-menu: inset 0 0 0 0.5px #ffffff26, 0 0 0 0.5px #000000b3, 0 10px 32px #00000099;
  --shadow-window: inset 0 0 0 0.5px #ffffff26, 0 0 0 0.5px #000000d9, 0 22px 56px #000000a6;
  --focus-ring: 0 0 0 3px #0a84ff;
}

@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    /* color */
    --window-bg: #323232;
    --content-bg: #1e1e1e;
    --content-bg-alt: #282828;
    --toolbar-bg: #363636eb;
    --sidebar-bg: #282828d9;
    --menu-bg: #2c2c2ceb;
    --control-bg: #636366;
    --control-bg-pressed: #4f4f51;
    --field-bg: #2c2c2e;
    --fill: #ffffff12;
    --fill-strong: #ffffff1f;
    --group-bg: #ffffff0d;
    --knob: #e5e5e7;
    --selection-inactive: #3a3a3a;
    --segment-selected: var(--control-bg);
    --separator: #ffffff1a;
    --control-border: #8e8e93;
    --label: #ffffffd9;
    --label-secondary: #ffffff8c;
    --label-tertiary: #ffffff40;
    --accent: var(--blue);
    --accent-fill: #0058d0;
    --link: #4aa3ff;
    --red: #ff453a;
    --orange: #ff9f0a;
    --yellow: #ffd60a;
    --green: #32d74b;
    --blue: #0a84ff;
    --purple: #bf5af2;
    --pink: #ff375f;
    --gray: #98989d;
    --red-text: #ff6961;
    --orange-text: #ffb340;
    --green-text: #30db5b;
    --danger-fill: #d92418;
    --window-control-inactive: #4e4e4e;
    --window-control-rim: #0000004d;
    /* shadow */
    --shadow-control: inset 0 0.5px 0 #ffffff26, 0 0 0 0.5px #00000066, 0 0.5px 1.5px #00000066;
    --shadow-field: inset 0 0.5px 1px #00000040;
    --shadow-knob: 0 0 0 0.5px #00000066, 0 1px 2.5px #00000080;
    --shadow-menu: inset 0 0 0 0.5px #ffffff26, 0 0 0 0.5px #000000b3, 0 10px 32px #00000099;
    --shadow-window: inset 0 0 0 0.5px #ffffff26, 0 0 0 0.5px #000000d9, 0 22px 56px #000000a6;
    --focus-ring: 0 0 0 3px #0a84ff;
  }
}

:root {
  /* spacing */
  --space-1: 4px;
  --space-2: 8px;
  --space-3: 12px;
  --space-4: 16px;
  --space-5: 20px;
  --space-6: 24px;
  --space-8: 32px;
  /* radius */
  --radius-xs: 3px;
  --radius-sm: 5px;
  --radius-md: 6px;
  --radius-lg: 10px;
  --radius-full: 9999px;
  /* size */
  --control-small: 18px;
  --control-regular: 22px;
  --control-large: 28px;
  --row-height: 24px;
  --sidebar-row: 28px;
  --titlebar-height: 28px;
  --toolbar-height: 52px;
  --sidebar-width: 220px;
  /* blur */
  --blur-material: 24px;
  --material-saturation: 180%;
  /* opacity */
  --opacity-disabled: 0.45;
  /* font families */
  --font-sans: -apple-system, BlinkMacSystemFont, "SF Pro Text", "SF Pro", "Helvetica Neue", "Segoe UI", system-ui, sans-serif;
  --font-mono: ui-monospace, "SF Mono", SFMono-Regular, Menlo, Monaco, Consolas, monospace;
}

/* text styles */
.pn-text-large-title { font-family: var(--font-sans); font-size: 26px; line-height: 32px; font-weight: 400; }
.pn-text-title-1 { font-family: var(--font-sans); font-size: 22px; line-height: 26px; font-weight: 400; }
.pn-text-title-2 { font-family: var(--font-sans); font-size: 17px; line-height: 22px; font-weight: 400; }
.pn-text-title-3 { font-family: var(--font-sans); font-size: 15px; line-height: 20px; font-weight: 400; }
.pn-text-headline { font-family: var(--font-sans); font-size: 13px; line-height: 16px; font-weight: 700; }
.pn-text-body { font-family: var(--font-sans); font-size: 13px; line-height: 16px; font-weight: 400; }
.pn-text-callout { font-family: var(--font-sans); font-size: 12px; line-height: 15px; font-weight: 400; }
.pn-text-subheadline { font-family: var(--font-sans); font-size: 11px; line-height: 14px; font-weight: 400; }
.pn-text-footnote { font-family: var(--font-sans); font-size: 10px; line-height: 13px; font-weight: 400; }
.pn-text-caption-1 { font-family: var(--font-sans); font-size: 10px; line-height: 13px; font-weight: 400; }
.pn-text-caption-2 { font-family: var(--font-sans); font-size: 10px; line-height: 13px; font-weight: 500; }
.pn-text-code { font-family: var(--font-mono); font-size: 12px; line-height: 16px; font-weight: 400; }
```

---

## Appendix B — Icon set

32 icons on a 16×16 grid. The format is `name → [element, attributes][]`. `fill: 1` means fill with `currentColor` and no stroke. `knockout: 1` means stroke with `var(--field-bg)`.

```json
{
  "sidebar": [["rect",{"x":1.75,"y":2.75,"width":12.5,"height":10.5,"rx":2}],["path",{"d":"M6 2.75 V13.25"}],["path",{"d":"M3.5 5.25 H4.25 M3.5 7.25 H4.25"}]],
  "chevron-left": [["path",{"d":"M10 3.5 L5.5 8 L10 12.5"}]],
  "chevron-right": [["path",{"d":"M6 3.5 L10.5 8 L6 12.5"}]],
  "chevron-down": [["path",{"d":"M3.5 6 L8 10.5 L12.5 6"}]],
  "chevron-up-down": [["path",{"d":"M5 6.25 L8 3.25 L11 6.25 M5 9.75 L8 12.75 L11 9.75"}]],
  "search": [["circle",{"cx":7,"cy":7,"r":4.5}],["path",{"d":"M10.4 10.4 L13.75 13.75"}]],
  "plus": [["path",{"d":"M8 3 V13 M3 8 H13"}]],
  "minus": [["path",{"d":"M3 8 H13"}]],
  "xmark": [["path",{"d":"M4 4 L12 12 M12 4 L4 12"}]],
  "xmark-circle": [["circle",{"cx":8,"cy":8,"r":6.5,"fill":1}],["path",{"d":"M5.75 5.75 L10.25 10.25 M10.25 5.75 L5.75 10.25","knockout":1}]],
  "checkmark": [["path",{"d":"M3.25 8.5 L6.5 11.75 L12.75 4.25"}]],
  "folder": [["path",{"d":"M1.75 4.5 A1.75 1.75 0 0 1 3.5 2.75 H6.25 L7.75 4.25 H12.5 A1.75 1.75 0 0 1 14.25 6 V11.5 A1.75 1.75 0 0 1 12.5 13.25 H3.5 A1.75 1.75 0 0 1 1.75 11.5 Z"}],["path",{"d":"M1.75 6.5 H14.25"}]],
  "doc": [["path",{"d":"M4.25 1.75 H9.25 L12.75 5.25 V13 A1.25 1.25 0 0 1 11.5 14.25 H4.25 A1.25 1.25 0 0 1 3 13 V3 A1.25 1.25 0 0 1 4.25 1.75 Z"}],["path",{"d":"M9.25 1.75 V5.25 H12.75"}],["path",{"d":"M5.5 8.5 H10.25 M5.5 11 H8.75"}]],
  "gear": [["circle",{"cx":8,"cy":8,"r":4.6}],["circle",{"cx":8,"cy":8,"r":1.9}],["path",{"d":"M12.60 8.00 L14.40 8.00 M11.25 11.25 L12.53 12.53 M8.00 12.60 L8.00 14.40 M4.75 11.25 L3.47 12.53 M3.40 8.00 L1.60 8.00 M4.75 4.75 L3.47 3.47 M8.00 3.40 L8.00 1.60 M11.25 4.75 L12.53 3.47"}]],
  "trash": [["path",{"d":"M2.75 4.25 H13.25"}],["path",{"d":"M6.25 4.25 V3 A0.75 0.75 0 0 1 7 2.25 H9 A0.75 0.75 0 0 1 9.75 3 V4.25"}],["path",{"d":"M4 4.25 L4.65 12.9 A1.25 1.25 0 0 0 5.9 14.05 H10.1 A1.25 1.25 0 0 0 11.35 12.9 L12 4.25"}],["path",{"d":"M6.75 7 V11.25 M9.25 7 V11.25"}]],
  "share": [["path",{"d":"M8 1.75 V9.75"}],["path",{"d":"M5.25 4.5 L8 1.75 L10.75 4.5"}],["path",{"d":"M5.5 6.75 H4.25 A1 1 0 0 0 3.25 7.75 V13.25 A1 1 0 0 0 4.25 14.25 H11.75 A1 1 0 0 0 12.75 13.25 V7.75 A1 1 0 0 0 11.75 6.75 H10.5"}]],
  "star": [["path",{"d":"M8.00 2.20 L9.62 6.38 L14.09 6.62 L10.62 9.45 L11.76 13.78 L8.00 11.35 L4.24 13.78 L5.38 9.45 L1.91 6.62 L6.38 6.38 Z"}]],
  "clock": [["circle",{"cx":8,"cy":8,"r":6.25}],["path",{"d":"M8 4.5 V8 L10.5 9.75"}]],
  "person": [["circle",{"cx":8,"cy":5.5,"r":2.75}],["path",{"d":"M2.75 14 A5.25 5.25 0 0 1 13.25 14"}]],
  "bell": [["path",{"d":"M4 11.25 V7.25 A4 4 0 0 1 12 7.25 V11.25 L13.25 12.5 H2.75 Z"}],["path",{"d":"M6.5 14.25 H9.5"}]],
  "tag": [["path",{"d":"M1.75 2.5 V7.4 A1 1 0 0 0 2.05 8.1 L8.15 14.2 A1 1 0 0 0 9.55 14.2 L14.2 9.55 A1 1 0 0 0 14.2 8.15 L8.1 2.05 A1 1 0 0 0 7.4 1.75 H2.5 A0.75 0.75 0 0 0 1.75 2.5 Z"}],["circle",{"cx":5,"cy":5,"r":1.1,"fill":1}]],
  "info": [["circle",{"cx":8,"cy":8,"r":6.25}],["path",{"d":"M8 7.25 V11.25"}],["circle",{"cx":8,"cy":4.85,"r":0.85,"fill":1}]],
  "warning": [["path",{"d":"M7.13 2.75 A1 1 0 0 1 8.87 2.75 L14.1 12.25 A1 1 0 0 1 13.23 13.75 H2.77 A1 1 0 0 1 1.9 12.25 Z"}],["path",{"d":"M8 6.25 V9.25"}],["circle",{"cx":8,"cy":11.4,"r":0.85,"fill":1}]],
  "ellipsis": [["circle",{"cx":3.5,"cy":8,"r":1.15,"fill":1}],["circle",{"cx":8,"cy":8,"r":1.15,"fill":1}],["circle",{"cx":12.5,"cy":8,"r":1.15,"fill":1}]],
  "list": [["path",{"d":"M5.75 4 H13.5 M5.75 8 H13.5 M5.75 12 H13.5"}],["circle",{"cx":2.75,"cy":4,"r":0.9,"fill":1}],["circle",{"cx":2.75,"cy":8,"r":0.9,"fill":1}],["circle",{"cx":2.75,"cy":12,"r":0.9,"fill":1}]],
  "grid": [["rect",{"x":2,"y":2,"width":5,"height":5,"rx":1.25}],["rect",{"x":9,"y":2,"width":5,"height":5,"rx":1.25}],["rect",{"x":2,"y":9,"width":5,"height":5,"rx":1.25}],["rect",{"x":9,"y":9,"width":5,"height":5,"rx":1.25}]],
  "download": [["path",{"d":"M8 2 V10.5"}],["path",{"d":"M4.75 7.25 L8 10.5 L11.25 7.25"}],["path",{"d":"M2.75 13.75 H13.25"}]],
  "cloud": [["path",{"d":"M4.6 12.75 H11.6 A3 3 0 0 0 12.1 6.8 A4.1 4.1 0 0 0 4.35 7.55 A2.6 2.6 0 0 0 4.6 12.75 Z"}]],
  "lock": [["rect",{"x":3.25,"y":7,"width":9.5,"height":7,"rx":1.75}],["path",{"d":"M5.25 7 V5 A2.75 2.75 0 0 1 10.75 5 V7"}]],
  "globe": [["circle",{"cx":8,"cy":8,"r":6.25}],["path",{"d":"M8 1.75 C5.25 4.5 5.25 11.5 8 14.25 C10.75 11.5 10.75 4.5 8 1.75 Z"}],["path",{"d":"M1.75 8 H14.25"}]],
  "photo": [["rect",{"x":1.75,"y":2.75,"width":12.5,"height":10.5,"rx":1.75}],["circle",{"cx":5.5,"cy":6.25,"r":1.15}],["path",{"d":"M1.75 11.25 L5.25 8 L8.25 10.75 L10.25 9 L14.25 12.5"}]],
  "house": [["path",{"d":"M2.25 7.25 L8 2.25 L13.75 7.25"}],["path",{"d":"M3.75 6 V13.25 H12.25 V6"}],["path",{"d":"M6.75 13.25 V9.75 H9.25 V13.25"}]]
}
```

Reference implementation:

```tsx
import { ICONS, type IconName } from './paths';

export function Icon({ name, size = 16, title, className, ...rest }:
  { name: IconName; size?: number; title?: string } & React.SVGAttributes<SVGSVGElement>) {
  const els = ICONS[name];
  return (
    <svg viewBox="0 0 16 16" width={size} height={size} fill="none" stroke="currentColor"
      strokeWidth={1.5} strokeLinecap="round" strokeLinejoin="round" className={className}
      role={title ? 'img' : undefined} aria-hidden={title ? undefined : true} focusable="false" {...rest}>
      {title ? <title>{title}</title> : null}
      {els.map(([tag, { fill, knockout, ...a }], i) => {
        const Tag = tag as 'path' | 'circle' | 'rect';
        const style = fill ? { fill: 'currentColor', stroke: 'none' } : knockout ? { stroke: 'var(--field-bg)' } : undefined;
        return <Tag key={i} {...(a as any)} style={style} />;
      })}
    </svg>
  );
}
```

`IconName` = 'sidebar' | 'chevron-left' | 'chevron-right' | 'chevron-down' | 'chevron-up-down' | 'search' | 'plus' | 'minus' | 'xmark' | 'xmark-circle' | 'checkmark' | 'folder' | 'doc' | 'gear' | 'trash' | 'share' | 'star' | 'clock' | 'person' | 'bell' | 'tag' | 'info' | 'warning' | 'ellipsis' | 'list' | 'grid' | 'download' | 'cloud' | 'lock' | 'globe' | 'photo' | 'house'.

---

## Appendix C — Reference stylesheet (Pane prototype)

This is the stylesheet of Pane's reference prototype: plain React components with global `pn-` classes, without headless primitives. Use it as a **source of exact values** and as a cross-check for §5. Don't copy its structure. Where the primitive provides state attributes, style those instead of the prototype's classes and ARIA selectors. The `.pn-demo*` helpers are for previews only.

```css
/* Pane — component styles. Every colour, radius, shadow and metric comes from tokens.css. */

:root, [data-theme="light"] { color-scheme: light; }
[data-theme="dark"] { color-scheme: dark; }

body {
  margin: 0;
  font-family: var(--font-sans);
  font-size: 13px;
  line-height: 16px;
  background: var(--window-bg);
  color: var(--label);
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

/* preview helpers */
.pn-demo { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-3); padding: var(--space-5); }
.pn-demo-col { display: flex; flex-direction: column; align-items: flex-start; gap: var(--space-3); padding: var(--space-5); }
.pn-demo-row { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-2); }
.pn-demo-content { background: var(--content-bg); }

.pn-icon { display: inline-block; flex: none; vertical-align: middle; }
.pn-icon-knockout { stroke: var(--field-bg); }

/* ---------------------------------------------------------------- Button */
.pn-btn {
  appearance: none; box-sizing: border-box; display: inline-flex; align-items: center; justify-content: center; gap: var(--space-1);
  height: var(--control-regular); padding: 0 var(--space-3); margin: 0;
  font: inherit; font-size: 13px; line-height: 16px; font-weight: 400; white-space: nowrap;
  color: var(--label); background: var(--control-bg); border: 0; border-radius: var(--radius-sm);
  box-shadow: var(--shadow-control); cursor: default; user-select: none; -webkit-user-select: none;
}
.pn-btn:active { background: var(--control-bg-pressed); }
.pn-btn:focus-visible { outline: none; box-shadow: var(--shadow-control), var(--focus-ring); }
.pn-btn-primary { background: var(--accent-fill); color: var(--on-accent); }
.pn-btn-primary:active { background: var(--accent-fill); filter: brightness(0.9); }
.pn-btn-destructive { background: var(--danger-fill); color: var(--on-danger); }
.pn-btn-destructive:active { background: var(--danger-fill); filter: brightness(0.9); }
.pn-btn-plain, .pn-btn-toolbar { background: transparent; box-shadow: none; }
.pn-btn-plain:hover, .pn-btn-toolbar:hover { background: var(--fill); }
.pn-btn-plain:active, .pn-btn-toolbar:active { background: var(--fill-strong); }
.pn-btn-plain:focus-visible, .pn-btn-toolbar:focus-visible { box-shadow: var(--focus-ring); }
.pn-btn-toolbar { height: var(--control-large); min-width: var(--control-large); padding: 0 var(--space-2); color: var(--label-secondary); border-radius: var(--radius-md); }
.pn-btn-toolbar[aria-pressed="true"] { background: var(--fill-strong); color: var(--label); }
.pn-btn-small { height: var(--control-small); padding: 0 var(--space-2); font-size: 11px; line-height: 14px; border-radius: 4px; }
.pn-btn-large { height: var(--control-large); padding: 0 var(--space-4); }
.pn-btn-icononly { padding: 0; width: var(--control-regular); }
.pn-btn-icononly.pn-btn-small { width: var(--control-small); }
.pn-btn-icononly.pn-btn-large, .pn-btn-icononly.pn-btn-toolbar { width: var(--control-large); }
.pn-btn:disabled { opacity: var(--opacity-disabled); filter: none; }
.pn-btn:disabled:active { background: var(--control-bg); }
.pn-btn-primary:disabled:active { background: var(--accent-fill); }
.pn-btn-plain:disabled, .pn-btn-toolbar:disabled { background: transparent; }

/* ------------------------------------------------------ SegmentedControl */
.pn-seg {
  display: inline-flex; box-sizing: border-box; height: var(--control-regular); padding: 1px;
  background: var(--fill-strong); border-radius: var(--radius-sm);
}
.pn-seg-item {
  appearance: none; position: relative; display: inline-flex; align-items: center; justify-content: center; gap: var(--space-1);
  margin: 0; padding: 0 var(--space-3); border: 0; border-radius: 4px; background: transparent;
  font: inherit; font-size: 13px; line-height: 16px; color: var(--label); cursor: default; white-space: nowrap;
}
.pn-seg-item + .pn-seg-item::before {
  content: ""; position: absolute; left: 0; top: 4px; bottom: 4px; width: 1px; background: var(--separator);
}
.pn-seg-item[aria-checked="true"] { background: var(--segment-selected); box-shadow: var(--shadow-control); z-index: 1; }
.pn-seg-item[aria-checked="true"]::before, .pn-seg-item[aria-checked="true"] + .pn-seg-item::before { display: none; }
.pn-seg-item:focus-visible { outline: none; box-shadow: var(--focus-ring); z-index: 2; }
.pn-seg-item:disabled { color: var(--label-tertiary); }
.pn-seg-small { height: var(--control-small); }
.pn-seg-small .pn-seg-item { font-size: 11px; line-height: 14px; padding: 0 var(--space-2); border-radius: 3px; }
.pn-seg-large { height: var(--control-large); }
.pn-seg-large .pn-seg-item { padding: 0 var(--space-4); }

/* ------------------------------------------------------- Text fields */
.pn-field { display: inline-flex; flex-direction: column; gap: var(--space-1); min-width: 0; }
.pn-field-label { font-size: 13px; line-height: 16px; color: var(--label); }
.pn-input {
  appearance: none; box-sizing: border-box; width: 100%; height: var(--control-regular); margin: 0; padding: 0 6px;
  font: inherit; font-size: 13px; line-height: 16px; color: var(--label);
  background: var(--field-bg); border: 0; border-radius: var(--radius-sm);
  box-shadow: inset 0 0 0 1px var(--control-border), var(--shadow-field);
}
.pn-input::placeholder { color: var(--label-secondary); opacity: 1; }
.pn-input:focus { outline: none; box-shadow: inset 0 0 0 1px var(--accent), var(--focus-ring); }
.pn-input:disabled { opacity: var(--opacity-disabled); }
.pn-input-small { height: var(--control-small); font-size: 11px; line-height: 14px; padding: 0 5px; border-radius: 4px; }
.pn-input-large { height: var(--control-large); padding: 0 var(--space-2); }
.pn-field.is-invalid .pn-input { box-shadow: inset 0 0 0 1px var(--red-text), var(--shadow-field); }
.pn-field.is-invalid .pn-input:focus { box-shadow: inset 0 0 0 1px var(--red-text), var(--focus-ring); }
.pn-field-msg { display: flex; align-items: center; gap: var(--space-1); font-size: 11px; line-height: 14px; color: var(--label-secondary); }
.pn-field.is-invalid .pn-field-msg { color: var(--red-text); }

.pn-search { position: relative; display: inline-flex; align-items: center; min-width: 0; }
.pn-search-icon { position: absolute; left: 7px; color: var(--label-secondary); pointer-events: none; }
.pn-search-input { padding-left: 25px; padding-right: 24px; border-radius: var(--radius-md); }
.pn-search-input::-webkit-search-cancel-button, .pn-search-input::-webkit-search-decoration { -webkit-appearance: none; appearance: none; }
.pn-search-clear {
  appearance: none; position: absolute; right: 4px; display: grid; place-items: center; width: 16px; height: 16px;
  margin: 0; padding: 0; border: 0; background: transparent; color: var(--label-secondary); cursor: default;
}
.pn-search-clear:hover { color: var(--label); }
.pn-search-large .pn-search-input { height: var(--control-large); padding-left: 28px; }
.pn-search-large .pn-search-icon { left: 8px; }
.pn-search-small .pn-search-input { height: var(--control-small); font-size: 11px; padding-left: 22px; border-radius: 4px; }
.pn-search-small .pn-search-icon { left: 6px; width: 11px; height: 11px; }

/* --------------------------------------------------- Checkbox & Radio */
.pn-check { position: relative; display: inline-flex; align-items: center; gap: 6px; font-size: 13px; line-height: 16px; color: var(--label); cursor: default; user-select: none; -webkit-user-select: none; }
.pn-check-input { position: absolute; left: 0; top: 50%; width: 14px; height: 14px; margin: -7px 0 0; opacity: 0; }
.pn-check-box {
  box-sizing: border-box; display: grid; place-items: center; flex: none; width: 14px; height: 14px;
  border-radius: var(--radius-xs); background: var(--field-bg); color: var(--on-accent);
  box-shadow: inset 0 0 0 1px var(--control-border), var(--shadow-field);
}
.pn-check-box svg { display: none; width: 12px; height: 12px; fill: none; stroke: currentColor; stroke-width: 1.9; stroke-linecap: round; stroke-linejoin: round; }
.pn-check-input:checked + .pn-check-box, .pn-check-input:indeterminate + .pn-check-box { background: var(--accent); box-shadow: none; }
.pn-check-input:checked:not(:indeterminate) + .pn-check-box .pn-check-mark { display: block; }
.pn-check-input:indeterminate + .pn-check-box .pn-check-dash { display: block; }
.pn-check-input:focus-visible + .pn-check-box,
.pn-check-input:focus-visible + .pn-radio-ring { box-shadow: inset 0 0 0 1px var(--control-border), var(--focus-ring); }
.pn-check-input:checked:focus-visible + .pn-check-box,
.pn-check-input:checked:focus-visible + .pn-radio-ring { box-shadow: var(--focus-ring); }
.pn-check.is-disabled { color: var(--label-tertiary); }
.pn-check.is-disabled .pn-check-box, .pn-check.is-disabled .pn-radio-ring { opacity: var(--opacity-disabled); }

.pn-radio-ring {
  box-sizing: border-box; display: grid; place-items: center; flex: none; width: 16px; height: 16px;
  border-radius: var(--radius-full); background: var(--field-bg);
  box-shadow: inset 0 0 0 1px var(--control-border), var(--shadow-field);
}
.pn-radio .pn-check-input { width: 16px; height: 16px; margin-top: -8px; }
.pn-radio-dot { width: 6px; height: 6px; border-radius: var(--radius-full); background: var(--on-accent); display: none; }
.pn-check-input:checked + .pn-radio-ring { background: var(--accent); box-shadow: none; }
.pn-check-input:checked + .pn-radio-ring .pn-radio-dot { display: block; }
fieldset.pn-radio-group { border: 0; margin: 0; padding: 0; display: flex; flex-direction: column; gap: var(--space-2); }
fieldset.pn-radio-group > legend { padding: 0; margin-bottom: var(--space-2); font-size: 13px; color: var(--label); }

/* ---------------------------------------------------------------- Switch */
.pn-switch {
  appearance: none; position: relative; box-sizing: border-box; flex: none; width: 38px; height: 22px; margin: 0; padding: 0;
  border: 0; border-radius: var(--radius-full); background: var(--fill-strong); cursor: default;
  box-shadow: inset 0 0 0 1px var(--control-border);
  transition: background-color .2s ease, box-shadow .2s ease;
}
.pn-switch-knob {
  position: absolute; top: 1px; left: 1px; width: 20px; height: 20px; border-radius: var(--radius-full);
  background: var(--knob); box-shadow: var(--shadow-knob); transition: transform .2s ease;
}
.pn-switch[aria-checked="true"] { background: var(--accent); box-shadow: none; }
.pn-switch[aria-checked="true"] .pn-switch-knob { transform: translateX(16px); }
.pn-switch:focus-visible { outline: none; box-shadow: inset 0 0 0 1px var(--control-border), var(--focus-ring); }
.pn-switch[aria-checked="true"]:focus-visible { box-shadow: var(--focus-ring); }
.pn-switch:disabled { opacity: var(--opacity-disabled); }
.pn-switch-small { width: 32px; height: 18px; }
.pn-switch-small .pn-switch-knob { width: 16px; height: 16px; }
.pn-switch-small[aria-checked="true"] .pn-switch-knob { transform: translateX(14px); }
.pn-switch-mini { width: 26px; height: 15px; }
.pn-switch-mini .pn-switch-knob { width: 13px; height: 13px; }
.pn-switch-mini[aria-checked="true"] .pn-switch-knob { transform: translateX(11px); }
.pn-switch-field { display: inline-flex; align-items: center; gap: var(--space-2); font-size: 13px; color: var(--label); cursor: default; }
.pn-switch-field.is-disabled { color: var(--label-tertiary); }

/* ------------------------------------------------------- Select (pop-up) */
.pn-select { position: relative; display: inline-flex; align-items: center; min-width: 0; }
.pn-select-native {
  appearance: none; -webkit-appearance: none; box-sizing: border-box; width: 100%; height: var(--control-regular); margin: 0;
  padding: 0 28px 0 var(--space-2); font: inherit; font-size: 13px; line-height: 16px; color: var(--label);
  background: var(--control-bg); border: 0; border-radius: var(--radius-sm); box-shadow: var(--shadow-control); cursor: default;
}
.pn-select-native:focus-visible { outline: none; box-shadow: var(--shadow-control), var(--focus-ring); }
.pn-select-arrows {
  position: absolute; right: 3px; top: 3px; bottom: 3px; width: 16px; display: grid; place-items: center;
  border-radius: var(--radius-xs); background: var(--accent-fill); color: var(--on-accent); pointer-events: none;
}
.pn-select-arrows .pn-icon { stroke-width: 2; }
.pn-select-small .pn-select-native { height: var(--control-small); font-size: 11px; padding-right: 22px; padding-left: 6px; border-radius: 4px; }
.pn-select-small .pn-select-arrows { width: 13px; top: 2px; bottom: 2px; right: 2px; }
.pn-select-large .pn-select-native { height: var(--control-large); padding-left: var(--space-3); padding-right: 32px; }
.pn-select-large .pn-select-arrows { width: 20px; top: 4px; bottom: 4px; right: 4px; }
.pn-select.is-disabled { opacity: var(--opacity-disabled); }

/* ---------------------------------------------------------------- Slider */
.pn-slider {
  -webkit-appearance: none; appearance: none; display: block; box-sizing: border-box; width: 200px; height: 20px; margin: 0; padding: 0;
  background: transparent; cursor: default;
}
.pn-slider::-webkit-slider-runnable-track {
  height: 4px; border-radius: var(--radius-full);
  background: linear-gradient(to right, var(--accent) 0 var(--pn-fill, 0%), var(--fill-strong) var(--pn-fill, 0%) 100%);
}
.pn-slider::-moz-range-track { height: 4px; border-radius: var(--radius-full); background: var(--fill-strong); }
.pn-slider::-moz-range-progress { height: 4px; border-radius: var(--radius-full); background: var(--accent); }
.pn-slider::-webkit-slider-thumb {
  -webkit-appearance: none; width: 20px; height: 20px; margin-top: -8px; border: 0; border-radius: var(--radius-full);
  background: var(--knob); box-shadow: var(--shadow-knob);
}
.pn-slider::-moz-range-thumb { width: 20px; height: 20px; border: 0; border-radius: var(--radius-full); background: var(--knob); box-shadow: var(--shadow-knob); }
.pn-slider:focus { outline: none; }
.pn-slider:focus-visible::-webkit-slider-thumb { box-shadow: var(--shadow-knob), var(--focus-ring); }
.pn-slider:focus-visible::-moz-range-thumb { box-shadow: var(--shadow-knob), var(--focus-ring); }
.pn-slider:disabled { opacity: var(--opacity-disabled); }
.pn-slider-small { height: 16px; }
.pn-slider-small::-webkit-slider-thumb { width: 15px; height: 15px; margin-top: -5.5px; }
.pn-slider-small::-moz-range-thumb { width: 15px; height: 15px; }

/* ------------------------------------------------------ Progress, Spinner */
.pn-progress { position: relative; box-sizing: border-box; width: 200px; height: 6px; border-radius: var(--radius-full); background: var(--fill-strong); overflow: hidden; }
.pn-progress-small { height: 4px; }
.pn-progress-fill { height: 100%; border-radius: inherit; background: var(--accent); transition: width .2s ease; }
.pn-progress.is-indeterminate .pn-progress-fill { position: absolute; top: 0; left: 0; width: 35%; animation: pn-indeterminate 1.4s ease-in-out infinite; }
@keyframes pn-indeterminate { 0% { transform: translateX(-100%); } 100% { transform: translateX(290%); } }

.pn-spinner { display: inline-block; flex: none; color: var(--label-secondary); line-height: 0; }
.pn-spinner svg { animation: pn-spin 1s steps(12) infinite; }
@keyframes pn-spin { to { transform: rotate(360deg); } }

@media (prefers-reduced-motion: reduce) {
  .pn-spinner svg { animation-duration: 3s; }
  .pn-progress.is-indeterminate .pn-progress-fill { animation-duration: 4s; }
  .pn-switch, .pn-switch-knob, .pn-progress-fill { transition: none; }
}

/* --------------------------------------------------------- Traffic lights */
.pn-lights { display: inline-flex; align-items: center; gap: 8px; flex: none; }
.pn-light {
  appearance: none; position: relative; display: grid; place-items: center; width: 12px; height: 12px; margin: 0; padding: 0;
  border: 0; border-radius: var(--radius-full); box-shadow: inset 0 0 0 0.5px var(--window-control-rim); cursor: default;
}
.pn-light-close { background: var(--window-close); }
.pn-light-minimize { background: var(--window-minimize); }
.pn-light-zoom { background: var(--window-zoom); }
.pn-light-glyph { width: 12px; height: 12px; fill: none; stroke: var(--window-control-glyph); stroke-width: 1.2; stroke-linecap: round; opacity: 0; }
.pn-light-glyph .pn-light-fill { fill: var(--window-control-glyph); stroke: none; }
.pn-lights:hover .pn-light-glyph, .pn-light:focus-visible .pn-light-glyph { opacity: 1; }
.pn-light:focus-visible { outline: none; box-shadow: var(--focus-ring); }
.pn-lights.is-inactive .pn-light { background: var(--window-control-inactive); }
.pn-lights.is-inactive:hover .pn-light-close { background: var(--window-close); }
.pn-lights.is-inactive:hover .pn-light-minimize { background: var(--window-minimize); }
.pn-lights.is-inactive:hover .pn-light-zoom { background: var(--window-zoom); }
.pn-light:disabled { background: var(--window-control-inactive); }
.pn-light:disabled .pn-light-glyph { opacity: 0; }

/* ---------------------------------------------------------------- Window */
.pn-window {
  --pn-sidebar-w: var(--sidebar-width);
  position: relative; box-sizing: border-box; display: flex; overflow: hidden; min-height: 160px;
  background: var(--content-bg); color: var(--label); border-radius: var(--radius-lg); box-shadow: var(--shadow-window);
  font-size: 13px; line-height: 16px;
}
.pn-window-sidebar {
  box-sizing: border-box; display: flex; flex-direction: column; flex: none; width: var(--pn-sidebar-w);
  background: var(--sidebar-bg);
  -webkit-backdrop-filter: blur(var(--blur-material)) saturate(var(--material-saturation));
  backdrop-filter: blur(var(--blur-material)) saturate(var(--material-saturation));
  box-shadow: inset -1px 0 0 var(--separator);
}
.pn-window-sidebar-top { display: flex; align-items: center; gap: var(--space-2); height: var(--toolbar-height); padding: 0 var(--space-3) 0 var(--space-5); flex: none; }
.pn-window-sidebar-top > :not(.pn-lights):first-of-type { margin-left: auto; }
.pn-window-sidebar-body { flex: 1; overflow: auto; padding-bottom: var(--space-3); }
.pn-window-main { display: flex; flex-direction: column; flex: 1; min-width: 0; }
.pn-window-toolbar {
  box-sizing: border-box; display: flex; align-items: center; gap: var(--space-3); flex: none;
  height: var(--toolbar-height); padding: 0 var(--space-3) 0 var(--space-5);
  background: var(--toolbar-bg);
  -webkit-backdrop-filter: blur(var(--blur-material)) saturate(var(--material-saturation));
  backdrop-filter: blur(var(--blur-material)) saturate(var(--material-saturation));
  box-shadow: inset 0 -1px 0 var(--separator);
}
.pn-window-toolbar > .pn-lights { margin-right: var(--space-2); }
.pn-window-titles { display: flex; flex-direction: column; justify-content: center; min-width: 0; }
.pn-window-title { font-size: 15px; line-height: 18px; font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.pn-window-subtitle { font-size: 11px; line-height: 14px; color: var(--label-secondary); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.pn-window-tools { display: flex; align-items: center; gap: var(--space-1); margin-left: auto; }
.pn-window-content { flex: 1; min-height: 0; overflow: auto; background: var(--content-bg); }
.pn-window.is-compact .pn-window-toolbar { height: var(--titlebar-height); padding: 0 var(--space-2); position: relative; }
.pn-window.is-compact .pn-window-toolbar > .pn-lights { margin-right: 0; }
.pn-window.is-compact .pn-window-titles { position: absolute; left: 70px; right: 70px; align-items: center; }
.pn-window.is-compact .pn-window-title { font-size: 13px; line-height: 16px; font-weight: 600; }
.pn-window.is-inactive .pn-window-title { color: var(--label-secondary); }
.pn-window.is-inactive .pn-window-toolbar, .pn-window.is-inactive .pn-window-sidebar { -webkit-backdrop-filter: none; backdrop-filter: none; }

/* --------------------------------------------------------------- Sidebar */
.pn-sidebar { display: flex; flex-direction: column; gap: var(--space-3); padding: 0 var(--space-2); }
.pn-sidebar ul { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; }
.pn-sidebar-heading { padding: 0 var(--space-2); margin-bottom: 2px; font-size: 11px; line-height: 20px; font-weight: 600; color: var(--label-secondary); }
.pn-sidebar-item {
  appearance: none; box-sizing: border-box; display: flex; align-items: center; gap: 7px; width: 100%; height: var(--sidebar-row);
  margin: 0; padding: 0 var(--space-2); border: 0; border-radius: var(--radius-md); background: transparent;
  font: inherit; font-size: 13px; line-height: 16px; color: var(--label); text-align: left; cursor: default;
}
.pn-sidebar-icon { color: var(--accent); width: 16px; height: 16px; }
.pn-sidebar-label { flex: 1; min-width: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.pn-sidebar-item[aria-current="page"] { background: var(--fill-strong); }
.pn-sidebar-item:focus-visible { outline: none; box-shadow: inset 0 0 0 2px var(--accent); }
.pn-sidebar-item:disabled { color: var(--label-tertiary); }
.pn-sidebar-item:disabled .pn-sidebar-icon { opacity: var(--opacity-disabled); }

/* ------------------------------------------------------------------ Menu */
.pn-menu {
  box-sizing: border-box; display: inline-flex; flex-direction: column; min-width: 200px; padding: 5px;
  background: var(--menu-bg); color: var(--label); border-radius: var(--radius-md); box-shadow: var(--shadow-menu);
  -webkit-backdrop-filter: blur(var(--blur-material)) saturate(var(--material-saturation));
  backdrop-filter: blur(var(--blur-material)) saturate(var(--material-saturation));
  font-size: 13px; line-height: 16px; outline: none; user-select: none; -webkit-user-select: none;
}
.pn-menu-item { display: flex; align-items: center; gap: 6px; height: 22px; padding: 0 var(--space-2) 0 var(--space-2); border-radius: 4px; cursor: default; outline: none; }
.pn-menu.has-checks .pn-menu-item { padding-left: 4px; }
.pn-menu-check { display: inline-grid; place-items: center; width: 14px; flex: none; }
.pn-menu-icon { color: var(--label-secondary); }
.pn-menu-label { flex: 1; white-space: nowrap; }
.pn-menu-shortcut { margin-left: var(--space-6); color: var(--label-secondary); letter-spacing: 0.04em; white-space: nowrap; }
.pn-menu-sub { color: var(--label-secondary); margin-left: var(--space-4); }
.pn-menu-item.is-active { background: var(--accent-fill); color: var(--on-accent); }
.pn-menu-item.is-active .pn-menu-shortcut, .pn-menu-item.is-active .pn-menu-sub, .pn-menu-item.is-active .pn-menu-icon { color: var(--on-accent); }
.pn-menu-item.is-disabled { color: var(--label-tertiary); }
.pn-menu-item.is-disabled .pn-menu-shortcut, .pn-menu-item.is-disabled .pn-menu-icon { color: var(--label-tertiary); }
.pn-menu-sep { height: 1px; margin: 5px var(--space-2); background: var(--separator); }
.pn-menu-heading { padding: 3px var(--space-2) 1px; font-size: 11px; line-height: 14px; font-weight: 600; color: var(--label-secondary); }

/* ----------------------------------------------------------------- Alert */
.pn-alert {
  box-sizing: border-box; display: flex; flex-direction: column; align-items: center; width: 260px; padding: var(--space-5) var(--space-4) var(--space-4);
  background: var(--window-bg); color: var(--label); border-radius: var(--radius-lg); box-shadow: var(--shadow-window); text-align: center;
}
.pn-alert-icon { display: grid; place-items: center; width: 64px; height: 64px; margin-bottom: var(--space-3); }
.pn-alert-title { margin: 0; font-size: 13px; line-height: 16px; font-weight: 700; }
.pn-alert-msg { margin: var(--space-2) 0 0; font-size: 11px; line-height: 14px; color: var(--label); }
.pn-alert-extra { margin-top: var(--space-3); align-self: stretch; display: flex; flex-direction: column; align-items: center; gap: var(--space-2); }
.pn-alert-actions { display: flex; gap: var(--space-2); align-self: stretch; margin-top: var(--space-4); }
.pn-alert-actions > .pn-btn { flex: 1; min-width: 0; }
.pn-alert-actions.is-stacked { flex-direction: column-reverse; }
.pn-alert-actions.is-stacked > .pn-btn { flex: none; }

/* -------------------------------------------------------------- GroupBox */
.pn-group { display: flex; flex-direction: column; gap: 6px; margin: 0; }
.pn-group-title { margin: 0 0 0 var(--space-2); font-size: 13px; line-height: 16px; font-weight: 700; color: var(--label); }
.pn-group-box { background: var(--group-bg); border-radius: var(--radius-lg); box-shadow: inset 0 0 0 0.5px var(--separator); }
.pn-group-footer { margin: 0 var(--space-2); font-size: 11px; line-height: 14px; color: var(--label-secondary); }
.pn-row { position: relative; display: flex; align-items: center; gap: var(--space-3); min-height: 40px; box-sizing: border-box; padding: var(--space-2) var(--space-3); }
.pn-row + .pn-row::before { content: ""; position: absolute; top: 0; left: var(--space-3); right: var(--space-3); height: 1px; background: var(--separator); }
.pn-row-icon { display: grid; place-items: center; flex: none; width: 22px; height: 22px; border-radius: var(--radius-sm); color: var(--on-accent); }
.pn-row-text { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
.pn-row-label { font-size: 13px; line-height: 16px; color: var(--label); }
.pn-row-desc { font-size: 11px; line-height: 14px; color: var(--label-secondary); }
.pn-row-control { display: flex; align-items: center; gap: var(--space-2); flex: none; }

/* ----------------------------------------------------------------- Table */
.pn-table-wrap { background: var(--content-bg); padding: 0 var(--space-2) var(--space-2); }
.pn-table { width: 100%; border-collapse: separate; border-spacing: 0; font-size: 13px; line-height: 16px; color: var(--label); outline: none; font-variant-numeric: tabular-nums; }
.pn-table th {
  position: relative; height: 28px; padding: 0 var(--space-2); text-align: left; font-weight: 500; font-size: 11px; color: var(--label-secondary);
  background: var(--content-bg); box-shadow: inset 0 -1px 0 var(--separator); white-space: nowrap;
}
.pn-table th + th::before { content: ""; position: absolute; left: 0; top: 7px; bottom: 7px; width: 1px; background: var(--separator); }
.pn-table td { height: var(--row-height); padding: 0 var(--space-2); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.pn-table td.is-secondary { color: var(--label-secondary); }
.pn-table tbody tr:nth-child(even) td { background: var(--content-bg-alt); }
.pn-table-wrap.no-stripes .pn-table tbody tr:nth-child(even) td { background: transparent; }
.pn-table tbody tr td:first-child { border-radius: var(--radius-sm) 0 0 var(--radius-sm); }
.pn-table tbody tr td:last-child { border-radius: 0 var(--radius-sm) var(--radius-sm) 0; }
.pn-table tbody tr.is-selected td, .pn-table tbody tr.is-selected:nth-child(even) td { background: var(--selection-inactive); }
.pn-table-wrap.is-focused .pn-table tbody tr.is-selected td { background: var(--accent-fill); color: var(--on-accent); }
.pn-table:focus-visible { box-shadow: inset 0 0 0 2px var(--accent); }

/* ----------------------------------------------------------------- Badge */
.pn-badge {
  box-sizing: border-box; display: inline-flex; align-items: center; justify-content: center; min-width: 18px; height: 16px; padding: 0 6px;
  border-radius: var(--radius-full); font-size: 11px; line-height: 14px; font-weight: 500; font-variant-numeric: tabular-nums; white-space: nowrap; flex: none;
}
.pn-badge-neutral { background: var(--fill-strong); color: var(--label); }
.pn-badge-accent { background: var(--accent-fill); color: var(--on-accent); }
.pn-badge-danger { background: var(--danger-fill); color: var(--on-danger); }

/* --------------------------------------------------------------- Tooltip */
.pn-tooltip-anchor { position: relative; display: inline-flex; }
.pn-tooltip {
  position: absolute; left: 0; z-index: 10; box-sizing: border-box; width: max-content; max-width: 280px; padding: 3px 7px;
  background: var(--menu-bg); color: var(--label); border-radius: var(--radius-md); box-shadow: var(--shadow-menu);
  -webkit-backdrop-filter: blur(var(--blur-material)) saturate(var(--material-saturation));
  backdrop-filter: blur(var(--blur-material)) saturate(var(--material-saturation));
  font-size: 12px; line-height: 15px; font-weight: 400; text-align: left; white-space: normal; pointer-events: none;
  visibility: hidden; opacity: 0; transition: opacity .12s ease, visibility 0s linear .12s;
}
.pn-tooltip-bottom { top: calc(100% + 6px); }
.pn-tooltip-top { bottom: calc(100% + 6px); }
.pn-tooltip.is-open { visibility: visible; opacity: 1; transition: opacity .12s ease; }
```
