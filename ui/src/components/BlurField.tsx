import { useEffect, useRef, useState } from "react";
import { Input } from "./ui/input";
import { Textarea } from "./ui/textarea";

/** A text field that keeps its own value while typing and saves when you leave it,
 *  so live updates from the server never jump the cursor. */
export function BlurField({ value, onSave, multiline, rows, ...rest }: {
  value: string; onSave: (v: string) => void; multiline?: boolean; rows?: number; placeholder?: string; id?: string; className?: string;
}) {
  const [v, setV] = useState(value);
  const focused = useRef(false);
  useEffect(() => {
    if (!focused.current) setV(value);
  }, [value]);
  const common = {
    value: v,
    onFocus: () => (focused.current = true),
    onBlur: () => {
      focused.current = false;
      if (v !== value) onSave(v);
    },
    ...rest,
  };
  return multiline ? (
    <Textarea rows={rows ?? 3} onChange={(e) => setV(e.target.value)} {...common} />
  ) : (
    <Input onChange={(e) => setV(e.target.value)} onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()} {...common} />
  );
}
