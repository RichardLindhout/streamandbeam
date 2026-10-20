import * as React from "react";
import { Select as P } from "radix-ui";
import { CheckIcon, ChevronDownIcon } from "lucide-react";
import { cn } from "@/lib/utils";

const Select = P.Root;
const SelectValue = P.Value;

function SelectTrigger({ className, children, ...props }: React.ComponentProps<typeof P.Trigger>) {
  return (
    <P.Trigger
      className={cn(
        "flex h-14 w-full items-center justify-between gap-2 rounded-xl border-2 border-input bg-card px-4 text-[22px] whitespace-nowrap shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/30 data-[placeholder]:text-muted-foreground [&>span]:truncate",
        className,
      )}
      {...props}
    >
      {children}
      <P.Icon asChild>
        <ChevronDownIcon className="size-6 opacity-70" />
      </P.Icon>
    </P.Trigger>
  );
}

function SelectContent({ className, children, position = "popper", ...props }: React.ComponentProps<typeof P.Content>) {
  return (
    <P.Portal>
      <P.Content
        position={position}
        className={cn(
          "relative z-50 max-h-[min(24rem,var(--radix-select-content-available-height))] min-w-[8rem] overflow-hidden rounded-xl border bg-popover text-popover-foreground shadow-2xl",
          position === "popper" && "w-full min-w-[var(--radix-select-trigger-width)] translate-y-1",
          className,
        )}
        {...props}
      >
        <P.Viewport className="scroll-thin p-1">{children}</P.Viewport>
      </P.Content>
    </P.Portal>
  );
}

function SelectItem({ className, children, ...props }: React.ComponentProps<typeof P.Item>) {
  return (
    <P.Item
      className={cn(
        "relative flex w-full cursor-default items-center min-h-14 rounded-lg py-2 pr-10 pl-4 text-[22px] outline-none select-none focus:bg-accent data-[disabled]:opacity-50",
        className,
      )}
      {...props}
    >
      <span className="absolute right-3 flex size-5 items-center justify-center">
        <P.ItemIndicator>
          <CheckIcon className="size-5 text-walnut" />
        </P.ItemIndicator>
      </span>
      <P.ItemText>{children}</P.ItemText>
    </P.Item>
  );
}

export { Select, SelectValue, SelectTrigger, SelectContent, SelectItem };
