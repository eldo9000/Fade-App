# Prompt Mission Checklist

Use this before every prompt-driven test run.

## Before You Start
- Confirm the mission targets only one format or one small UI area.
- Confirm the output folder is already clear, or explicitly allow stale files.
- Confirm whether file renaming is allowed.
- Confirm the exact file selection rule:
  - use any valid visible file, or
  - use one specific file, or
  - stop if the choice is ambiguous.

## During the Run
- Select the intended file first.
- Open the intended format panel.
- Change only the settings named in the mission.
- Click the exact conversion control named in the mission.
- Do not invent new suffixes or rename outputs unless the mission says to.
- Do not branch into unrelated tabs or controls.
- If the button is missing, disabled, or seems to do nothing, stop and report the UI state.

## After Each Conversion
- Check whether the UI reflects the action you just took.
- Check whether the expected output file appeared.
- Note the exact setting combination that was used.
- Note any mismatch between the UI and the result.

## Report Rules
- Report the button you clicked.
- Report the file that was used.
- Report the exact setting combination.
- Report the output file name, or say that no file appeared.
- Report any blocker as a blocker, not as a guessed success.

## Don’t Do These Unless Asked
- Do not rename files.
- Do not create new suffixes.
- Do not retry with a different file just to keep moving.
- Do not change unrelated settings.
- Do not fix bugs unless the mission explicitly authorizes fixes.
