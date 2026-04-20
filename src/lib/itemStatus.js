/**
 * @typedef {'pending'|'converting'|'done'|'error'|'cancelled'} ItemStatus
 *
 * @typedef {{
 *   id: string,
 *   kind: 'file',
 *   parentId: string|null,
 *   path: string,
 *   name: string,
 *   ext: string,
 *   mediaType: 'video'|'audio'|'image'|'data'|'document'|'archive'|'model'|'timeline'|'subtitle'|'ebook'|'email'|'font'|'unknown',
 *   status: ItemStatus,
 *   percent: number,
 *   error: string|null,
 *   outputPath: string|null,
 *   info: {duration_secs?: number, width?: number, height?: number, file_size?: number}|null,
 * }} QueueFileItem
 */

/** @param {QueueFileItem} item */
export function markConverting(item) {
  item.status  = 'converting';
  item.percent = 0;
  item.error   = null;
}

/** @param {QueueFileItem} item @param {unknown} err */
export function markError(item, err) {
  item.status = 'error';
  item.error  = String(err);
}

/** @param {QueueFileItem} item @param {string} [outputPath] */
export function markDone(item, outputPath) {
  item.status  = 'done';
  item.percent = 100;
  if (outputPath != null) item.outputPath = outputPath;
}

/** @param {QueueFileItem} item @param {number} percent */
export function markProgress(item, percent) {
  item.status  = 'converting';
  item.percent = percent;
}

/** @param {QueueFileItem} item */
export function markCancelled(item) {
  item.status  = 'cancelled';
  item.percent = 0;
}
