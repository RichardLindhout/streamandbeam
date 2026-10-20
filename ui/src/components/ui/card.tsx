import * as React from "react";
import { cn } from "@/lib/utils";

function Card({ className, ...props }: React.ComponentProps<"div">) {
  return <div data-slot="card" className={cn("rounded-2xl border-2 bg-card text-card-foreground", className)} {...props} />;
}

function CardHeader({ className, children, action, ...props }: React.ComponentProps<"div"> & { action?: React.ReactNode }) {
  return (
    <div data-slot="card-header" className={cn("relative flex min-h-16 items-center gap-3 border-b-2 px-6 py-3", className)} {...props}>
      <div className="flex min-w-0 flex-1 items-center gap-3 font-serif text-[26px] font-semibold [&_svg]:size-6 [&_svg]:text-walnut">{children}</div>
      {action}
    </div>
  );
}

function CardContent({ className, ...props }: React.ComponentProps<"div">) {
  return <div data-slot="card-content" className={cn("p-6", className)} {...props} />;
}

export { Card, CardHeader, CardContent };
