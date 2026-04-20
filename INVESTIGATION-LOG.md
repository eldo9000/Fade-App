2026-04-16 | CONFIRMED | output-format-driven right panel — dispatch 1 green, commit 1ea385a
2026-04-17 | OPEN | dispatch 1/3 — viz chevrons inverted (UP when closed, content expands below) + canvases black (no _connectSource on expand)
2026-04-17 | CONFIRMED | viz chevrons inverted + no _connectSource on expand — dispatch 1 green
2026-04-20 | CONFIRMED | 10 mutex .lock().unwrap() sites → .expect() — dispatch 1 green, commit c547715
2026-04-20 | CONFIRMED | extract UpdateManager, PresetManager, CropEditor from App.svelte — dispatch 1 green, commits 5ca633a 0a593a0 a13e6c1
2026-04-20 | OPEN | dispatch 1/3 — extract QueueManager from App.svelte; critical path is preserving _loadGen cancellation pattern and $bindable selectedItem chain
