#!/usr/bin/env python3
"""
Computer use test harness — reusable across any macOS desktop app.

The harness drives Claude's computer use API in a screenshot→action loop.
It reads a JSON manifest of scenarios, executes each one, and reports results.

Dependencies:
    pip install anthropic pyautogui pillow

macOS permissions required (System Settings → Privacy & Security):
    Accessibility     — mouse + keyboard control
    Screen Recording  — screenshots

Usage:
    export ANTHROPIC_API_KEY=sk-...
    python harness.py scenarios/fade.json
    python harness.py scenarios/fade.json --filter image
    python harness.py scenarios/fade.json --id img-png-to-webp
    python harness.py scenarios/fade.json --verbose
"""

import anthropic
import argparse
import base64
import io
import json
import os
import subprocess
import sys
import time
from pathlib import Path

import pyautogui
from PIL import Image

# ── Screenshot ────────────────────────────────────────────────────────────────

def take_screenshot() -> tuple[str, int, int]:
    """
    Capture the screen in logical coordinates (matches pyautogui's coordinate
    space). Returns (base64_png, width, height).
    """
    img = pyautogui.screenshot()
    buf = io.BytesIO()
    img.save(buf, format="PNG")
    data = base64.standard_b64encode(buf.getvalue()).decode("utf-8")
    return data, img.width, img.height


# ── Action executor ───────────────────────────────────────────────────────────

# Map X11 key names (what the API returns) to pyautogui key names
KEY_MAP = {
    "Return": "enter", "KP_Enter": "enter",
    "Escape": "esc", "Tab": "tab",
    "BackSpace": "backspace", "Delete": "delete",
    "space": "space",
    "Left": "left", "Right": "right", "Up": "up", "Down": "down",
    "Home": "home", "End": "end", "Page_Up": "pageup", "Page_Down": "pagedown",
    "F1": "f1", "F2": "f2", "F3": "f3", "F4": "f4",
    "F5": "f5", "F6": "f6", "F7": "f7", "F8": "f8",
}


def execute_action(action: dict) -> None:
    t = action.get("type")

    if t == "mouse_move":
        x, y = action["coordinate"]
        pyautogui.moveTo(x, y, duration=0.15)

    elif t == "left_click":
        x, y = action["coordinate"]
        pyautogui.click(x, y)

    elif t == "right_click":
        x, y = action["coordinate"]
        pyautogui.rightClick(x, y)

    elif t == "double_click":
        x, y = action["coordinate"]
        pyautogui.doubleClick(x, y)

    elif t == "left_click_drag":
        sx, sy = action["start_coordinate"]
        ex, ey = action["coordinate"]
        pyautogui.moveTo(sx, sy, duration=0.1)
        pyautogui.dragTo(ex, ey, duration=0.4, button="left")

    elif t == "type":
        text = action["text"]
        pyautogui.write(text, interval=0.03)

    elif t == "key":
        key_str = action["text"]
        # Handle modifier combos like "ctrl+a", "super+v", "shift+Tab"
        parts = key_str.split("+")
        mods = []
        key = parts[-1]
        for mod in parts[:-1]:
            if mod == "ctrl":
                mods.append("command")  # macOS: ctrl → command for most actions
            elif mod == "super":
                mods.append("command")
            elif mod == "alt":
                mods.append("option")
            elif mod == "shift":
                mods.append("shift")
        key = KEY_MAP.get(key, key.lower())
        if mods:
            pyautogui.hotkey(*mods, key)
        else:
            pyautogui.press(key)

    elif t == "scroll":
        x, y = action["coordinate"]
        direction = action.get("direction", "down")
        amount = int(action.get("amount", 3))
        # pyautogui.scroll: positive = up, negative = down
        clicks = amount if direction == "up" else -amount
        pyautogui.scroll(clicks, x=x, y=y)

    time.sleep(0.35)


# ── Scenario runner ───────────────────────────────────────────────────────────

SYSTEM_PROMPT = """\
You are an automated GUI test agent operating a macOS desktop application.

Your job is to complete the given task by interacting with the UI via the \
computer tool — take screenshots, click, type, scroll as needed.

Rules:
- Always take a screenshot first to see the current state.
- Click precisely on UI elements; prefer the center of buttons and controls.
- After triggering a file conversion, wait for the job to show completion \
(progress indicator gone, status shows success or done) before declaring done.
- If a system dialog or error popup appears, dismiss it and continue.
- Do not close the application window.
- When the task is fully and verifiably complete, respond with exactly: DONE
- If you cannot complete the task after multiple attempts, respond with: \
FAILED: <one sentence reason>
"""


def run_scenario(
    client: anthropic.Anthropic,
    scenario: dict,
    screen_w: int,
    screen_h: int,
    verbose: bool = False,
) -> dict:
    task = scenario["task"]
    assertion = scenario.get("assertion")
    max_steps = scenario.get("max_steps", 40)

    computer_tool = {
        "type": "computer_20241022",
        "name": "computer",
        "display_width_px": screen_w,
        "display_height_px": screen_h,
    }

    # Seed with the task text
    messages: list[dict] = [
        {"role": "user", "content": task}
    ]

    steps = 0
    result_text = ""

    while steps < max_steps:
        response = client.beta.messages.create(
            model="claude-opus-4-7",
            max_tokens=4096,
            system=SYSTEM_PROMPT,
            tools=[computer_tool],
            messages=messages,
            betas=["computer-use-2024-10-22"],
        )

        messages.append({"role": "assistant", "content": response.content})

        tool_results = []
        terminal = False

        for block in response.content:
            if not hasattr(block, "type"):
                continue

            if block.type == "text":
                result_text = block.text
                if verbose:
                    print(f"      agent: {block.text[:100]}")
                if "DONE" in block.text or "FAILED" in block.text:
                    terminal = True

            elif block.type == "tool_use" and block.name == "computer":
                action = block.input
                action_type = action.get("type", "?")

                if verbose:
                    coord = action.get("coordinate", action.get("text", ""))
                    print(f"      [{steps:02d}] {action_type} {coord}")

                if action_type == "screenshot":
                    img_data, _, _ = take_screenshot()
                    tool_results.append({
                        "type": "tool_result",
                        "tool_use_id": block.id,
                        "content": [{
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": "image/png",
                                "data": img_data,
                            },
                        }],
                    })
                else:
                    execute_action(action)
                    steps += 1
                    img_data, _, _ = take_screenshot()
                    tool_results.append({
                        "type": "tool_result",
                        "tool_use_id": block.id,
                        "content": [{
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": "image/png",
                                "data": img_data,
                            },
                        }],
                    })

        if terminal:
            break

        if tool_results:
            messages.append({"role": "user", "content": tool_results})

        if response.stop_reason == "end_turn" and not tool_results:
            break

    # Evaluate
    if steps >= max_steps and "DONE" not in result_text:
        result_text = f"Max steps ({max_steps}) reached"

    passed = "DONE" in result_text and "FAILED" not in result_text

    if passed and assertion:
        check = subprocess.run(assertion, shell=True, capture_output=True)
        if check.returncode != 0:
            passed = False
            result_text = (
                f"Agent said DONE but assertion failed: {assertion}\n"
                f"stdout: {check.stdout.decode().strip()}\n"
                f"stderr: {check.stderr.decode().strip()}"
            )

    return {
        "passed": passed,
        "steps": steps,
        "result": result_text,
    }


# ── Entry point ───────────────────────────────────────────────────────────────

def main() -> None:
    parser = argparse.ArgumentParser(description="Computer use test harness")
    parser.add_argument("manifest", help="Path to scenarios JSON file")
    parser.add_argument("--filter", help="Only run scenarios in this category")
    parser.add_argument("--id", help="Only run the scenario with this id")
    parser.add_argument("--verbose", "-v", action="store_true",
                        help="Print each action as it executes")
    args = parser.parse_args()

    api_key = os.environ.get("ANTHROPIC_API_KEY")
    if not api_key:
        print("Error: ANTHROPIC_API_KEY not set", file=sys.stderr)
        sys.exit(1)

    manifest_path = Path(args.manifest)
    if not manifest_path.exists():
        print(f"Error: {manifest_path} not found", file=sys.stderr)
        sys.exit(1)

    manifest = json.loads(manifest_path.read_text())
    scenarios = manifest["scenarios"]

    # Substitute fixture_dir placeholder
    fixture_dir = manifest.get("fixture_dir", "")
    if fixture_dir.startswith("."):
        fixture_dir = str((manifest_path.parent / fixture_dir).resolve())
    for s in scenarios:
        s["task"] = s["task"].replace("{{fixture_dir}}", fixture_dir)
        if "assertion" in s:
            s["assertion"] = s["assertion"].replace("{{fixture_dir}}", fixture_dir)

    if args.filter:
        scenarios = [s for s in scenarios if s.get("category") == args.filter]
    if args.id:
        scenarios = [s for s in scenarios if s.get("id") == args.id]

    if not scenarios:
        print("No scenarios matched.")
        sys.exit(1)

    # Detect screen size once
    _, screen_w, screen_h = take_screenshot()

    client = anthropic.Anthropic(api_key=api_key)

    # Run app setup if specified
    if "setup" in manifest:
        print(f"Setup: {manifest['setup']}")
        subprocess.run(manifest["setup"], shell=True)
        time.sleep(3)

    print(f"\n{manifest.get('app', 'App')} — {len(scenarios)} scenario(s)\n")

    results = []
    for scenario in scenarios:
        sid = scenario.get("id", "?")
        name = scenario.get("name", scenario.get("task", "")[:55])
        print(f"  {sid}  {name}")

        result = run_scenario(client, scenario, screen_w, screen_h, verbose=args.verbose)
        result["id"] = sid
        results.append(result)

        status = "✓" if result["passed"] else "✗"
        snippet = result["result"].splitlines()[0][:70]
        print(f"    {status}  {result['steps']} steps — {snippet}")

    passed = sum(1 for r in results if r["passed"])
    total = len(results)
    print(f"\n{passed}/{total} passed\n")

    if passed < total:
        print("Failed:")
        for r in results:
            if not r["passed"]:
                print(f"  {r['id']}: {r['result'].splitlines()[0]}")
        sys.exit(1)


if __name__ == "__main__":
    main()
