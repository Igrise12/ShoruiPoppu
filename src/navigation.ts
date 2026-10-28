export function slideTarget(key: string, index: number, count: number): number | null {
  if (count <= 0) return null;
  switch (key) {
    case "ArrowRight": case "PageDown": return Math.min(index + 1, count - 1);
    case "ArrowLeft": case "PageUp": return Math.max(index - 1, 0);
    case "Home": return 0;
    case "End": return count - 1;
    default: return null;
  }
}

export function pdfPage(value: string, count: number): number | null {
  if (!/^\d+$/.test(value.trim()) || count < 1) return null;
  const page = Number(value);
  return Number.isSafeInteger(page) ? Math.min(count, Math.max(1, page)) : null;
}
