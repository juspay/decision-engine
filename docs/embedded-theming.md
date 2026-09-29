# Embedded dashboard themes

The HS routing workspace can pass its resolved merchant theme into a DE iframe opened with `embed=1`. HS owns the sidebar, header, logo, theme selection, and inheritance. DE applies the content theme without fetching theme configuration or storing merchant branding in local/session storage.

V1 uses light mode. Standalone DE retains its existing light/dark preference. Deploy the DE receiver before enabling the HS sender; either application can fall back when the other does not support themes.

## Message contract

Both documents must share an origin, including protocol and port. Use a same-origin development proxy when running separate servers. DE accepts messages only from `window.parent`; HS must check the current iframe's `contentWindow`. Always supply an exact `targetOrigin`. A portal embedding HS passes its configuration through HS's existing `init_config` flow; it must not send theme updates directly to the nested DE frame.

DE installs the listener before React renders and announces a unique ID for each frame document:

```json
{"type":"de:theme-ready","version":1,"frameId":"<unique document ID>"}
```

HS responds with a complete snapshot, and sends another snapshot when its resolved theme changes:

```json
{
  "type": "de:theme-update",
  "version": 1,
  "frameId": "<unique document ID>",
  "revision": 1,
  "tokens": {
    "primary": "#7138a8",
    "background": "#f2f0f7",
    "surface": "#ffffff",
    "primaryButtonBackground": "#24639a",
    "primaryButtonText": "#ffffff",
    "primaryButtonHover": "#194b76",
    "fontFamily": "Roboto, sans-serif",
    "fontSize": "14px",
    "headingFontSize": "24px",
    "radius": "4px"
  }
}
```

Revisions are nonnegative safe integers increasing within the host provider's lifetime. DE ignores lower revisions, mismatched frame IDs, unsupported versions, and messages with the wrong origin/source. Duplicate revisions are safe to resend. The acknowledgment is `{type: "de:theme-applied", version: 1, frameId, revision}`. It is informational and does not replace the existing authentication/route/session messages.

HS can send `{type: "de:theme-request", version: 1}` on iframe load to request readiness again. This handles listener timing and in-frame reloads without polling. Theme-only updates never change the iframe URL or discard form state.

DE holds its content for at most one second while awaiting the initial snapshot, then reveals the default light theme. A valid late snapshot still applies. The pending document is transparent so the host's iframe background shows through. A new frame starts fresh; logout or an in-app merchant switch clears overrides and stops the old listener. The host remints/reloads the frame for a new authenticated scope.

## Tokens and HS mapping

Every field is optional. A snapshot replaces all previous overrides, so `tokens: {}` restores DE defaults. Invalid individual values are omitted; a non-object payload is ignored. Unknown fields have no effect. The payload is limited to 32 keys and accepts no raw CSS, selectors, asset URLs, or executable content.

| DE token | HS source / behavior |
| --- | --- |
| `primary` | `settings.colors.primary`; supplies the brand ramp for focus, selected controls, accents, and highlighted chart outcomes |
| `background` | `settings.colors.background`; page and loading surfaces |
| `surface` | HS adapter uses white for content cards, controls, and overlays |
| `primaryButtonBackground`, `primaryButtonText`, `primaryButtonHover` | `settings.buttons.primary.backgroundColor`, `textColor`, `hoverBackgroundColor` |
| `secondaryButtonBackground`, `secondaryButtonText`, `secondaryButtonHover` | Corresponding `settings.buttons.secondary` fields |
| `link`, `linkHover` | `settings.typography.linkColor`, `linkHoverColor`; branded anchors |
| `fontFamily` | `settings.typography.fontFamily`; supported stacks below |
| `fontSize` | `settings.typography.fontSize`; root rem scale, 12–20px |
| `headingFontSize` | `settings.typography.headingFontSize`; page headings, 18–36px |
| `radius` | `settings.borders.defaultRadius`; shared buttons, cards, dialogs, and form fields, 0–24px |
| `text`, `mutedText`, `border` | Optional neutral-role overrides; the HS adapter leaves DE's readable defaults intact |

Colors must be six-digit hex values (`#RRGGBB`). Sizes use `px`, with at most two decimal places. Supported font stacks are `Roboto, sans-serif`, `Inter, sans-serif`, `Arial, sans-serif`, `Helvetica, Arial, sans-serif`, and `system-ui, sans-serif`. Font availability still depends on the deployment's font loading policy; arbitrary uploaded fonts are not supported.

Brand colors and button colors are separate. Semantic success/warning/error colors, categorical chart series, and code syntax highlighting keep their existing meanings. The HS editor currently exposes brand/sidebar/button controls; typography and radius come from the broader theme configuration. Sidebar and logo settings are not duplicated inside DE.

Choose colors with readable text contrast and visible focus states. V1 does not infer a dark palette, replace the theme editor, or expose free-form CSS/layout customization.

## Verification

`tests/e2e/smoke/embed-theme.spec.ts` uses real iframe documents and the DE app with mocked auth/routing responses. It covers live updates with an unsaved form, control styling, message validation, reset, fallback, reloads, session rejection, concurrent frames, and standalone preferences.

Run the frontend build and Playwright typecheck, then the focused UI spec against a running frontend. The test mocks its API calls and can use `PW_NO_WEBSERVER=1` with `UI_BASE_URL` when no Rust server is needed. The CC companion suite lives in `playwright-tests/e2e/6-workflow/DecisionEngineWorkspace.spec.ts` and requires its normal HS test stack.
