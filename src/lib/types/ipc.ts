// Typed IPC helpers — field-level references to generated types.
// If a Rust field renames, cargo test regenerates the .ts files, and
// tsc --noEmit breaks here. That is the intended behavior.

import type { JobProgress } from "./generated/JobProgress";
import type { JobDone } from "./generated/JobDone";
import type { JobError } from "./generated/JobError";
import type { JobCancelled } from "./generated/JobCancelled";
import type { ConvertOptions } from "./generated/ConvertOptions";

export type IpcEvents = {
    "job-progress": JobProgress;
    "job-done": JobDone;
    "job-error": JobError;
    "job-cancelled": JobCancelled;
};

// Exhaustive field assertions — never called, exist only for tsc.
// A Rust field rename regenerates the .ts file with the new name,
// breaking these assignments at the old name.
function _assertJobProgress(p: JobProgress): void {
    const _id: string = p.job_id;
    const _pct: number = p.percent;
    const _msg: string = p.message;
    void _id; void _pct; void _msg;
}

function _assertJobDone(p: JobDone): void {
    const _id: string = p.job_id;
    const _path: string = p.output_path;
    void _id; void _path;
}

function _assertJobError(p: JobError): void {
    const _id: string = p.job_id;
    const _msg: string = p.message;
    void _id; void _msg;
}

function _assertJobCancelled(p: JobCancelled): void {
    const _id: string = p.job_id;
    void _id;
}

function _assertConvertOptions(o: ConvertOptions): void {
    const _fmt: string = o.output_format;
    const _dir: string | null = o.output_dir;
    const _quality: number | null = o.quality;
    const _codec: string | null = o.codec;
    void _fmt; void _dir; void _quality; void _codec;
}
