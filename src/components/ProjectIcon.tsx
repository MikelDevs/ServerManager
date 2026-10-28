import { useState } from "react";
import { isProjectImage } from "../utils/projectImage";

interface ProjectIconProps {
  icon: string | null;
  name: string;
  className: string;
}

export function ProjectIcon({ icon, name, className }: ProjectIconProps) {
  const [failedSource, setFailedSource] = useState<string | null>(null);
  const image = isProjectImage(icon);
  const fallback = icon && !image && !icon.startsWith("data:")
    ? icon
    : name.trim().slice(0, 2).toLocaleUpperCase() || "▰";

  return (
    <span className={`${className} project-image-frame`} aria-hidden="true">
      {icon && image && failedSource !== icon ? (
        <img src={icon} alt="" onError={() => setFailedSource(icon)} />
      ) : (
        <span className="project-icon-fallback">{fallback}</span>
      )}
    </span>
  );
}
