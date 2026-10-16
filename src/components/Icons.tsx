import type { SVGProps } from "react";

type IconProps = SVGProps<SVGSVGElement>;

function IconBase({ children, ...props }: IconProps) {
  return (
    <svg
      aria-hidden="true"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
      {...props}
    >
      {children}
    </svg>
  );
}

export const PlusIcon = (props: IconProps) => (
  <IconBase {...props}><path d="M12 5v14M5 12h14" /></IconBase>
);
export const PlayIcon = (props: IconProps) => (
  <IconBase {...props}><path d="m8 5 11 7-11 7Z" /></IconBase>
);
export const StopIcon = (props: IconProps) => (
  <IconBase {...props}><rect x="6" y="6" width="12" height="12" rx="2" /></IconBase>
);
export const RestartIcon = (props: IconProps) => (
  <IconBase {...props}><path d="M20 7v5h-5" /><path d="M19 12a7 7 0 1 1-2-5" /></IconBase>
);
export const EditIcon = (props: IconProps) => (
  <IconBase {...props}><path d="m4 20 4-.8L18.6 8.6a2 2 0 0 0-2.8-2.8L5.2 16.4Z" /><path d="m14 7 3 3" /></IconBase>
);
export const TrashIcon = (props: IconProps) => (
  <IconBase {...props}><path d="M4 7h16M9 7V4h6v3m3 0-1 13H7L6 7M10 11v5m4-5v5" /></IconBase>
);
export const TerminalIcon = (props: IconProps) => (
  <IconBase {...props}><rect x="3" y="4" width="18" height="16" rx="3" /><path d="m7 9 3 3-3 3m6 0h4" /></IconBase>
);
export const FolderIcon = (props: IconProps) => (
  <IconBase {...props}><path d="M3 7a2 2 0 0 1 2-2h5l2 2h7a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z" /></IconBase>
);
export const ClockIcon = (props: IconProps) => (
  <IconBase {...props}><circle cx="12" cy="12" r="9" /><path d="M12 7v5l3 2" /></IconBase>
);
export const CloseIcon = (props: IconProps) => (
  <IconBase {...props}><path d="m6 6 12 12M18 6 6 18" /></IconBase>
);
export const ChevronIcon = (props: IconProps) => (
  <IconBase {...props}><path d="m9 18 6-6-6-6" /></IconBase>
);
export const CodeIcon = (props: IconProps) => (
  <IconBase {...props}><path d="m9 18-6-6 6-6m6 0 6 6-6 6" /><path d="m14 4-4 16" /></IconBase>
);
export const BrowserIcon = (props: IconProps) => (
  <IconBase {...props}><rect x="3" y="4" width="18" height="16" rx="3" /><path d="M3 9h18M7 6.5h.01M10 6.5h.01" /></IconBase>
);
