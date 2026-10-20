import { useEffect, useLayoutEffect, useRef } from "react";
import type { Slide } from "@/lib/api";
import { cn } from "@/lib/utils";

/** One board, drawn by the same renderSlide as the TVs (slide.js), scaled to its frame. */
export function Board({ slide, black, className }: { slide: Slide | null | undefined; black?: boolean; className?: string }) {
  const frame = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLDivElement>(null);
  const draw = () => {
    if (canvas.current && typeof renderSlide === "function") renderSlide(canvas.current, slide, black, { portrait: true });
  };
  useLayoutEffect(draw);
  useEffect(() => {
    if (!frame.current) return;
    const ro = new ResizeObserver(() => draw());
    ro.observe(frame.current);
    return () => ro.disconnect();
  });
  return (
    <div ref={frame} className={cn("mon aspect-[9/16]", className)}>
      <div className="canvas" ref={canvas} />
    </div>
  );
}
