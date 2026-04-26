# MP4 Pilot Missions

## Mission A: Basic Conversion Obedience
Prompt:
- Convert sample MP4 to WebM with default options and report pass/fail with evidence.

Checks:
- File selection succeeds.
- Output format switch to WebM is retained.
- Conversion completes.
- Output exists and is non-empty.

## Mission B: Codec Stress
Prompt:
- In MP4/video options, cycle codecs (H.264/H.265/AV1/VP9 where available) and check for state-reset bugs.

Checks:
- Codec change reflects in UI state.
- Invalid combos are prevented or warned.
- No silent reset of unrelated controls.

## Mission C: Resolution/FPS Persistence
Prompt:
- Change resolution and frame rate repeatedly; verify selections persist through convert action.

Checks:
- Resolution setting persists.
- Frame-rate setting persists.
- Conversion output matches intended settings where measurable.

## Mission D: Error-Surface Quality
Prompt:
- Trigger one intentional invalid setup and evaluate error clarity + recoverability.

Checks:
- Error surfaced to user.
- Message is actionable.
- User can recover without restarting app.
