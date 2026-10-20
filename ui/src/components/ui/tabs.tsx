import * as React from "react";
import { Tabs as P } from "radix-ui";
import { cn } from "@/lib/utils";

const Tabs = P.Root;

function TabsList({ className, ...props }: React.ComponentProps<typeof P.List>) {
  return <P.List className={cn("flex shrink-0 gap-2 overflow-x-auto border-b px-7", className)} {...props} />;
}

function TabsTrigger({ className, ...props }: React.ComponentProps<typeof P.Trigger>) {
  return (
    <P.Trigger
      className={cn(
        "-mb-px border-b-4 border-transparent px-4 py-4 text-[22px] font-semibold whitespace-nowrap text-muted-foreground transition hover:text-foreground data-[state=active]:border-walnut data-[state=active]:text-foreground",
        className,
      )}
      {...props}
    />
  );
}

const TabsContent = P.Content;

export { Tabs, TabsList, TabsTrigger, TabsContent };
