import { Toaster as Sonner, type ToasterProps } from "sonner";

function Toaster(props: ToasterProps) {
  return (
    <Sonner
      theme="dark"
      position="bottom-center"
      toastOptions={{
        classNames: {
          toast: "!bg-popover !border-border !text-foreground !rounded-xl !shadow-2xl !text-[14px]",
          actionButton: "!bg-primary !text-[#1b150d] !font-semibold !rounded-md !px-3 !h-8",
        },
      }}
      {...props}
    />
  );
}

export { Toaster };
