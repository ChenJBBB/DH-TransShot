/** Extract the sentence around the exact textarea selection (UTF-16 offsets). */
export function selectionContext(text: string, start: number, end: number): string {
  const boundary = /[。！？.!?\n]/;
  const preceding = Array.from(text.slice(0, start));
  const following = Array.from(text.slice(end));
  const before: string[] = [];
  const after: string[] = [];
  for (let i = preceding.length - 1; i >= 0 && before.length < 160; i--) {
    if (boundary.test(preceding[i])) break;
    before.push(preceding[i]);
  }
  for (const character of following) {
    if (after.length >= 160) break;
    after.push(character);
    if (boundary.test(character)) break;
  }
  return (before.reverse().join("") + text.slice(start, end) + after.join("")).trim();
}
