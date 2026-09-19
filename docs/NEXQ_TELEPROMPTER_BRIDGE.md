# NexQ → EyeRead Teleprompter Bridge

EyeRead exposes a localhost-only HTTP bridge for NexQ.

## Endpoint

`POST http://127.0.0.1:17842/v1/teleprompter/script`

JSON body:

```json
{"text":"The answer text to read.","title":"NexQ Answer","language":"en-US","id":"optional-id"}
```

Only `text` is required.

A successful request returns `{"ok":true}`.

The bridge forwards the payload directly into EyeRead's existing `overlay:load` path. That means the existing overlay placement, styling, voice matching, scrolling, and share-protection behavior remain the source of truth.

## Quick local test

With EyeRead running, open its overlay once, then from PowerShell:

```powershell
Invoke-RestMethod `
  -Uri http://127.0.0.1:17842/v1/teleprompter/script `
  -Method Post `
  -ContentType 'application/json' `
  -Body (@{
    text = 'Hello, this is a NexQ teleprompter test.'
    title = 'Bridge Test'
    language = 'en-US'
  } | ConvertTo-Json)
```

The overlay should load the text and start reading immediately.

## NexQ integration contract

NexQ only needs to POST the generated answer. It does not need to know anything about EyeRead's React state, voice matcher, overlay position, or scrolling.

Recommended flow:

`NexQ answer generation → POST /v1/teleprompter/script → EyeRead overlay:load → existing voice tracking`

The existing NexQ-fixed installation is not modified by this branch.