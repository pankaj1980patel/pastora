import { useEffect, useRef, useState } from "react";

/** Keeps a selected index in range and scrolls the selected row into view. */
export function useListNav(count: number, resetKey: unknown) {
  const [selected, setSelected] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);
  /** Back to the first row, scrolled to the top (also when row 0 was already selected). */
  const reset = () => {
    setSelected(0);
    if (listRef.current) listRef.current.scrollTop = 0;
  };
  useEffect(reset, [resetKey]);
  useEffect(() => {
    if (selected >= count) setSelected(Math.max(0, count - 1));
  }, [count, selected]);
  useEffect(() => {
    const el = listRef.current?.querySelector<HTMLElement>(`[data-selected="true"]`);
    el?.scrollIntoView({ block: "nearest" });
  }, [selected]);
  const move = (delta: number) => setSelected((s) => Math.max(0, Math.min(count - 1, s + delta)));
  return { selected, setSelected, move, listRef, reset };
}
