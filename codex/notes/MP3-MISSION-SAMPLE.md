# MP3 Mission Sample

## Mission Name
MP3 Settings Sweep and Conversion Check

## Purpose
Use a tightly scoped prompt-driven agent run to walk through the MP3 panel, change one setting at a time, convert the visible file, and record any UI or output errors.

This mission is intentionally narrow so the agent does not need to reason about multiple formats, multiple files, or unrelated controls.

## Assumptions
- The queue contains at least one valid audio file, but the exact one does not matter.
- The app is already open on the correct dev instance.
- The output folder is cleared before the run.
- The mission uses the app's normal output naming.
- The MP3 panel is the only active target for this mission.

## Phase 1: Sweep
The first agent should:
1. Open the MP3 output panel.
2. Pick any valid audio file already visible in the queue.
3. Confirm the file is selected and the MP3 panel is visible.
4. Change one setting at a time.
5. Click the exact `Convert Selected` button after each setting change.
6. Record whether the UI state stayed correct and whether the output was created.
7. Continue until all MP3 output options in scope have been exercised.
8. Stop immediately if a control behaves unexpectedly or if the agent cannot tell which file is valid.

## Settings To Sweep
- Bitrate
- Sample rate
- Bitrate mode
- Channels
- Trim start / trim end
- Preserve metadata

## Phase 1 Prompt Template
```text
Open the MP3 panel and use any valid audio file already visible in the queue.
Change one MP3 setting at a time, convert after each change, and confirm the output is created.
For every setting, note whether the UI state stayed correct and whether the resulting file was written.
I do not care whether you use the WAV or the MP3 source file, only that you use a valid sample media file.
Do not rename files, create new suffixes, or change unrelated settings.
Use the exact `Convert Selected` button for each test.
Stop if the file choice is ambiguous, the button is missing, or any setting does not stick.
Report each setting change, the button you clicked, the output file name, and any error you encounter.
```

## Phase 1 Pass Criteria
- The agent visits each MP3 setting in scope.
- Each setting is converted at least once.
- The agent reports all failures, odd UI behavior, or missing outputs.
- The report clearly states which setting was being tested when the problem happened.

## Phase 2: Confirmation
The second agent should:
1. Read the sweep report.
2. Verify the reported output files exist.
3. Confirm the reported UI states match expectations.
4. Re-run only the failed or suspicious cases.
5. Produce a final pass/fail summary.

## Phase 2 Prompt Template
```text
Review the MP3 sweep report and verify each reported output file exists and looks correct.
Re-run only the failed or suspicious cases.
Confirm whether each issue is real, fixed, or a false alarm.
Report the final status for each MP3 setting.
```

## Notes
- Keep the mission to one format only.
- Use whichever valid audio file is easiest, WAV or MP3.
- Avoid switching to video, image, archive, or data controls during this run.
- If the queue contains more than one plausible target file, stop and report the ambiguity instead of guessing.
- If conversion is blocked because a file already exists, do not work around it by renaming; report it as a stale-output issue.
