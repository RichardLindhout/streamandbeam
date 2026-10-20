import * as React from "react";
import { DropdownMenu as P } from "radix-ui";
import { cn } from "@/lib/utils";

const DropdownMenu = P.Root;
const DropdownMenuTrigger = P.Trigger;

function DropdownMenuContent({ className, sideOffset = 6, align = "end", ...props }: React.ComponentProps<typeof P.Content>) {
  return (
    <P.Portal>
      <P.Content
        sideOffset={sideOffset}
        align={align}
        className={cn("z-50 min-w-[16rem] overflow-hidden rounded-xl border bg-popover p-1.5 text-popover-foreground shadow-2xl", className)}
        {...props}
      />
    </P.Portal>
  );
}

function DropdownMenuItem({ className, ...props }: React.ComponentProps<typeof P.Item>) {
  return (
    <P.Item
      className={cn(
        "relative flex cursor-default items-center gap-3 rounded-lg px-3 py-2.5 text-[15px] outline-none select-none focus:bg-accent data-[disabled]:opacity-50 [&_svg]:size-4 [&_svg]:text-primary",
        className,
      )}
      {...props}
    />
  );
}

function DropdownMenuSeparator({ className, ...props }: React.ComponentProps<typeof P.Separator>) {
  return <P.Separator className={cn("my-1.5 h-px bg-border", className)} {...props} />;
}

export { DropdownMenu, DropdownMenuTrigger, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator };
