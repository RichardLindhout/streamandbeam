import * as React from "react";
import { cn } from "@/lib/utils";

function Card({ className, ...props }: React.ComponentProps<"div">) {
  return <div data-slot="card" className={cn("rounded-2xl border bg-card text-card-foreground shadow-[0_1px_0_rgb(239_230_214/0.04)_inset,0_24px_44px_-26px_rgb(0_0_0/0.9)]", className)} {...props} />;
}

function CardHeader({ className, children, action, ...props }: React.ComponentProps<"div"> & { action?: React.ReactNode }) {
  return (
    <div data-slot="card-header" className={cn("relative flex min-h-12 items-center gap-3 border-b px-5 py-3 after:absolute after:-bottom-px after:left-5 after:h-px after:w-12 after:bg-primary/80", className)} {...props}>
      <div className="hymnal flex min-w-0 flex-1 items-center gap-2.5 text-[15px] text-[#cbbb9f] [&_svg]:size-4 [&_svg]:text-primary/80">{children}</div>
      {action}
    </div>
  );
}

function CardContent({ className, ...props }: React.ComponentProps<"div">) {
  return <div data-slot="card-content" className={cn("p-5", className)} {...props} />;
}

export { Card, CardHeader, CardContent };
