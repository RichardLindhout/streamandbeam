import * as React from "react";
import { Dialog as P } from "radix-ui";
import { XIcon } from "lucide-react";
import { cn } from "@/lib/utils";

const Sheet = P.Root;

/** A full-height panel from the right, for long lists. */
function SheetContent({ className, title, children, ...props }: React.ComponentProps<typeof P.Content> & { title: string }) {
  return (
    <P.Portal>
      <P.Overlay className="fixed inset-0 z-50 bg-[#2b1d12]/55" />
      <P.Content
        aria-describedby={undefined}
        className={cn("fixed inset-y-0 right-0 z-50 flex w-[min(640px,100%)] flex-col border-l-2 bg-popover shadow-2xl outline-none", className)}
        {...props}
      >
        <div className="flex h-24 shrink-0 items-center gap-4 border-b-2 pr-4 pl-7 max-md:h-auto max-md:min-h-16 max-md:pt-[env(safe-area-inset-top)] max-md:pr-2 max-md:pl-4">
          <P.Title className="flex-1 font-serif text-[30px] font-semibold max-md:text-[24px]">{title}</P.Title>
          <P.Close className="grid size-[72px] place-items-center rounded-xl transition hover:bg-accent max-md:size-14">
            <XIcon className="size-8" />
            <span className="sr-only">Sluiten</span>
          </P.Close>
        </div>
        <div className="scroll-thin min-h-0 flex-1 overflow-y-auto p-3 max-md:p-2 max-md:pb-[max(0.5rem,env(safe-area-inset-bottom))]">{children}</div>
      </P.Content>
    </P.Portal>
  );
}

export { Sheet, SheetContent };
