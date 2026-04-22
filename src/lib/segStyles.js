// Shared segmented-control button class helpers.
// Horizontal row: seg(active, i, total)
// Vertical stack: segV(active, i, total)
// Both return Tailwind class strings for connected segmented buttons.

export function seg(active, i, total) {
  const base = 'px-3 py-[5px] text-center text-[12px] font-medium border transition-colors relative';
  const round = i === 0 ? 'rounded-l-md' : i === total - 1 ? 'rounded-r-md' : '';
  const overlap = i > 0 ? '-ml-px' : '';
  const color = active
    ? 'seg-active z-10'
    : 'seg-inactive border-[var(--border)] text-[color-mix(in_srgb,var(--text-primary)_70%,transparent)] hover:z-10';
  return [base, round, overlap, color].filter(Boolean).join(' ');
}

export function segV(active, i, total) {
  const base  = 'w-full px-3 py-[5px] text-left text-[12px] font-medium border transition-colors relative';
  const round = i === 0 ? 'rounded-t-md' : i === total - 1 ? 'rounded-b-md' : '';
  const mt    = i > 0 ? '-mt-px' : '';
  const color = active
    ? 'seg-active z-10'
    : 'seg-inactive border-[var(--border)] text-[color-mix(in_srgb,var(--text-primary)_70%,transparent)] hover:z-10';
  return [base, round, mt, color].filter(Boolean).join(' ');
}
