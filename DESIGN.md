# Longcut Design System

## 0. Research Log

- Embedded references: shortlisted Runway (cinematic media), Sentry (dense operations), and Linear (precise hierarchy) → picked operational `taste-skill` + Runway because the editor must disappear behind real footage while remaining information-dense.
- Lazyweb: skipped — external product-screen network research is unavailable in this execution lane; the frozen product contract and curated local reference are authoritative.
- Imagen drafts: skipped — no image-generation tool is available; imported source thumbnails and video are the visual material, never fabricated imagery.
- Contract grounding: `docs/implementation-contract.md` defines the actual entities, commands, media behavior, and honest preview/render labels.

## 1. Atmosphere & Identity

Longcut is a quiet, film-production-grade cutting room: charcoal stone surfaces, precise labels, and dense controls recede behind the work. Its signature is the **cut line** — a sharp lime playhead crossing violet scene material, always making the distinction between current time, hover skimmer, and selected range unmistakable. Design variance 5, motion intensity 3, visual density 8.

Primary persona: an editor assembling long recordings who needs speed, trustworthy state, and frame-level clarity. Secondary persona: a reviewer leaving exact range or pin feedback. Both must understand what is approximate, what is revision-pinned, and what will be rendered.

## 2. Color

Dark mode is the product mode; footage supplies the chroma. All values are CSS custom properties.

| Role | Token | Value | Usage |
|---|---|---|---|
| Canvas | `--ink-0` | `#0c0d0d` | App background |
| Recessed | `--ink-1` | `#121313` | Timeline and wells |
| Panel | `--ink-2` | `#191a19` | Primary panels |
| Raised | `--ink-3` | `#222321` | Controls and selected rows |
| Hover | `--ink-4` | `#2b2c29` | Interactive hover |
| Edge | `--line-1` | `#30312e` | Structural rules |
| Strong edge | `--line-2` | `#454641` | Focus-adjacent borders |
| Primary text | `--paper-1` | `#f0f1e9` | Titles and values |
| Secondary text | `--paper-2` | `#b8bab0` | Body and labels |
| Muted text | `--paper-3` | `#81837b` | Metadata |
| Lime | `--lime-1` | `#c7f36b` | Primary action, playhead, focus |
| Lime strong | `--lime-2` | `#ddff92` | Hover and active accent |
| Lime ink | `--lime-ink` | `#18200c` | Text on lime |
| Violet | `--violet-1` | `#7968b2` | Scene clips |
| Violet strong | `--violet-2` | `#9a87dc` | Selected scene and ranges |
| Violet dark | `--violet-ink` | `#29223e` | Clip depth |
| Success | `--success` | `#7fd59a` | Ready/applied |
| Warning | `--warning` | `#e2bd68` | Processing/stale |
| Error | `--error` | `#ef817d` | Error/destructive |

Rules: lime is reserved for present-time and primary action; violet belongs to editorial material. Status colors never masquerade as clickable accents. No other literal colors appear in UI styles.

## 3. Typography

The interface uses a single deliberate sans family, **IBM Plex Sans**, with IBM Plex Mono for frame values. Fonts use local/system fallbacks to remain offline-capable.

| Token | Size / line | Weight | Usage |
|---|---|---|---|
| `--type-display` | 32px / 1.05 | 500 | Empty-state brand moment |
| `--type-h1` | 24px / 1.15 | 500 | Project title |
| `--type-h2` | 18px / 1.25 | 500 | Panel heading |
| `--type-h3` | 15px / 1.3 | 600 | Card/scene title |
| `--type-body` | 14px / 1.45 | 400 | Default UI copy |
| `--type-small` | 12px / 1.4 | 450 | Secondary copy |
| `--type-micro` | 11px / 1.3 | 550 | Uppercase labels |

Frame counts, revisions, dimensions, and timecodes use the mono family. Visible body copy never drops below 12px; interactive labels are at least 12px.

## 4. Spacing & Layout

Base unit is 4px. Tokens: `--s1` 4px, `--s2` 8px, `--s3` 12px, `--s4` 16px, `--s5` 20px, `--s6` 24px, `--s8` 32px, `--s10` 40px, `--s12` 48px.

Editor shell: 100dvh bounded grid with fixed 52px top bar, fluid workspace, and timeline region. The workspace owns scroll; panel internals only scroll when their job is explicit. Wide layout uses a 228px source rail, fluid monitor/story area, and 292px inspector. The Agent workspace uses a 320px connection/run rail and a fluid request surface. Below 1080px the inspector becomes an in-flow lower panel. Below 760px all workspaces form a readable single column; navigation and timeline toolbars horizontally scroll rather than crushing labels. No primary content forces viewport horizontal scrolling.

Geometry tokens: `--r1` 4px, `--r2` 8px, `--r3` 12px, `--agent-rail` 320px, `--agent-content` 760px. Controls use `--r1`; panels and media frames use `--r2`; home cards use `--r3`. The Agent request keeps a readable maximum measure while its rail remains compact.

## 5. Components

Media artboards retain a strict 16:9 content coordinate space at every viewport;
camera geometry and comment coordinates must not include decorative letterboxing.
The mobile header reserves separate 52px identity and 40px navigation rows.
Exact render review supports independent side-by-side version playback, not claimed
frame-synchronized A/B playback.

### Button / IconButton
- **Structure**: semantic button, icon from Lucide, optional label.
- **Variants**: primary lime, secondary raised, ghost, destructive.
- **Spacing**: `--s2` / `--s3`; 36px minimum visual height, 44px touch target on narrow layouts.
- **States**: hover changes tonal surface, active translates 1px, focus gets 2px lime outline, disabled dims, loading keeps label width.
- **Accessibility**: text/icon contrast, `aria-label` on icon-only controls, native keyboard behavior.
- **Motion**: micro token, transform/opacity only.

### Panel
- **Structure**: heading cluster, optional tools, scroll body.
- **Variants**: standard, recessed, selected.
- **States**: empty and error are inline; loading uses restrained text, never fake progress.
- **Layout**: stack primitive; the body is the declared scroll owner where necessary.

### Field
- **Structure**: visible label, input/textarea/select, helper or error below.
- **States**: default, hover, focus, disabled, error.
- **Accessibility**: labels are never placeholders; IME composition is never intercepted.

### StatusBadge
- **Structure**: status dot plus literal state label.
- **Variants**: neutral, processing, ready, error, synthetic.
- **Accessibility**: never communicates by color alone.

### MediaFrame
- **Structure**: 16:9 frame, explicit Source/Program label, media element or cinematic empty state, transport and timecode.
- **States**: empty, processing, ready, error, approximate scene preview, exact revision-pinned render.
- **Layout**: frame + overlay-stack; media reserves its ratio to avoid layout shift.

### Timeline
- **Structure**: ruler, separate skimmer, lime playhead, violet clip track, range overlay, clip labels and edit toolbar.
- **States**: scene default/hover/selected/locked/manual; playhead and skimmer must never share a visual style or state variable.
- **Accessibility**: range control supports keyboard; scene buttons expose title and frame range.
- **Motion**: state transitions only; timeline geometry does not animate while scrubbing.

### TabBar
- **Structure**: nav list and buttons for Sources, Story, Edit, Agent, Review, Deliverables.
- **States**: active uses lime underline and primary text; hover/focus are distinct.
- **Layout**: cluster/reel; narrow widths scroll inline with keyboard access.

### Dialog / Drawer
- **Structure**: heading, clear body, grouped fields, cancel/confirm footer.
- **States**: validation and API errors remain contextual.
- **Accessibility**: native dialog semantics where used, focus visible, Escape closes unless a submission is active.

### AgentConnection
- **Structure**: literal Codex installation, version, authentication mode, and device-flow state; official authorization link and public one-time code only.
- **States**: checking, unavailable, logged out, device login pending, ChatGPT ready, busy, unsupported API authentication, and error.
- **Trust**: never renders credential, token, API-key, provider, or custom-endpoint inputs. Pending login can be cancelled and connected sessions can be logged out.

### AgentProviderSelector
- **Structure**: always-visible two-option selector followed by the selected provider's status surface. Codex composes the existing `AgentConnection`; OpenAI-compatible API shows only server-reported configuration, public model/endpoint, and `.env` setting names.
- **States**: checking, selected, configured, and unavailable. A configured API provider may be selected before Codex is connected; provider failures never change the selection or trigger fallback.
- **Trust**: no secret value, API-key field, model override, endpoint override, or browser persistence. API billing is explicitly separate from ChatGPT subscription usage.

### AgentRequest
- **Structure**: source/scenes segmented scope, bounded source interval or unlocked scene checklist, exact context disclosure, instruction, consent, and primary request action.
- **States**: usable with an empty timeline when a ready source exists; empty-source and empty-scene guidance; validation is inline and submission never clears a draft unless it succeeds or the user explicitly resets.
- **Trust**: states that the first release uses supplied metadata and at most 12 sampled visual frames, with no ASR, audio understanding, or semantic review.

### AiRunList
- **Structure**: newest-first run records with literal status, frozen revision, scope, sample count, summary, warnings, errors, cancel/review/start-over actions.
- **States**: queued, sampling, generating, ready, error, cancelled, and stale. Only active states poll. A ready proposal links into the existing partial-approval Review workspace.
- **Truthfulness**: each run displays its frozen provider, but no percentage, fabricated stage, or production mock fallback is displayed.

### Primitive Showcase
The `/showcase` query route renders every primitive in default, selected, disabled, empty, processing, and error states. It shares production CSS and is the implementation harness for mobile/tablet/desktop checks.

## 6. Motion & Interaction

| Token | Duration | Easing | Usage |
|---|---|---|---|
| `--motion-micro` | 120ms | ease-out | press/hover/focus |
| `--motion-standard` | 220ms | cubic-bezier(.2,.8,.2,1) | panel/tab state |

Motion communicates state only. Timeline input never depends on animation. Reduced-motion removes transforms and transitions. Keyboard shortcuts ignore editable targets and `event.isComposing`; transport uses Space only outside form controls, I/O mark source range, Delete removes selected scene, Mod+Z undo, Mod+Shift+Z redo.

## 7. Depth & Surface

Strategy: **tonal shift with structural hairlines, zero drop shadows**. Like a grading suite, depth comes from controlled luminance and the footage itself. Raised overlays use `--ink-3` and `--line-2`; panels use `--ink-2`; wells use `--ink-1`. A subtle fixed noise layer is permitted only as a non-scrolling, pointer-inert pseudo-element at very low opacity.

## 8. Accessibility Constraints & Accepted Debt

Target WCAG 2.2 AA: body contrast 4.5:1, large/UI graphics 3:1, visible focus, complete keyboard reachability, semantic controls, reduced motion, and 44px coarse-pointer targets. Timeline colors include labels and shape changes. Error messages identify the action that failed. API state is never represented by fabricated success, AI activity, or progress.

| Item | Location | Why accepted | Owner / Exit |
|---|---|---|---|
| Browser-level visual QA and screen-reader walkthrough pending | Whole UI | Backend and browser runner are integrated by the lead after all services land | Lead runs `/visual-qa` against the integrated stack |
| Drag-to-reorder is supplemented by Move earlier/later buttons | Timeline | Deterministic keyboard editing ships first | Add accessible drag-and-drop after user testing |
