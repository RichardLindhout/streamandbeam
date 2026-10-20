import * as React from "react";
import { cn } from "@/lib/utils";

function Card({ className, ...props }: React.ComponentProps<"div">) {
  return <div data-slot="card" className={cn("rounded-2xl border bg-card text-card-foreground shadow-[0_1px_0_rgb(255_255_255/0.03)_inset,0_20px_40px_-24px_rgb(0_0_0/0.8)]", className)} {...props} />;
}

function CardHeader({ className, children, action, ...props }: React.ComponentProps<"div"> & { action?: React.ReactNode }) {
  return (
    <div data-slot="card-header" className={cn("flex min-h-12 items-center gap-3 border-b px-5 py-3", className)} {...props}>
      <div className="flex min-w-0 flex-1 items-center gap-2.5 text-[11px] font-bold uppercase tracking-[0.14em] text-muted-foreground [&_svg]:size-4">{children}</div>
      {action}
    </div>
  );
}

function CardContent({ className, ...props }: React.ComponentProps<"div">) {
  return <div data-slot="card-content" className={cn("p-5", className)} {...props} />;
}

export { Card, CardHeader, CardContent };
