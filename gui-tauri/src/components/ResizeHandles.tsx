import type { MouseEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

type ResizeDirection = Parameters<
  ReturnType<typeof getCurrentWindow>["startResizeDragging"]
>[0];

interface ResizeHandlesProps {
  disabled: boolean;
}

const HANDLES: ReadonlyArray<{
  direction: ResizeDirection;
  className: string;
}> = [
  { direction: "North", className: "top-0 right-2.5 left-2.5 h-[4px] cursor-n-resize" },
  { direction: "South", className: "bottom-0 right-2.5 left-2.5 h-[4px] cursor-s-resize" },
  { direction: "West", className: "top-2.5 bottom-2.5 left-0 w-[4px] cursor-w-resize" },
  { direction: "East", className: "top-2.5 bottom-2.5 right-0 w-[4px] cursor-e-resize" },
  { direction: "NorthWest", className: "top-0 left-0 h-2.5 w-2.5 cursor-nw-resize" },
  { direction: "NorthEast", className: "top-0 right-0 h-2.5 w-2.5 cursor-ne-resize" },
  { direction: "SouthWest", className: "bottom-0 left-0 h-2.5 w-2.5 cursor-sw-resize" },
  { direction: "SouthEast", className: "bottom-0 right-0 h-2.5 w-2.5 cursor-se-resize" },
];

export function ResizeHandles({ disabled }: ResizeHandlesProps) {
  if (disabled) {
    return null;
  }

  const handleMouseDown = (
    event: MouseEvent<HTMLDivElement>,
    direction: ResizeDirection
  ) => {
    if (event.button !== 0) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    void getCurrentWindow()
      .startResizeDragging(direction)
      .catch((err) => console.error(`startResizeDragging(${direction}) failed:`, err));
  };

  return (
    <div aria-hidden="true" className="pointer-events-none fixed inset-0 z-50">
      {HANDLES.map(({ direction, className }) => (
        <div
          key={direction}
          data-resize-direction={direction}
          onMouseDown={(e) => handleMouseDown(e, direction)}
          className={`pointer-events-auto fixed ${className}`}
        />
      ))}
    </div>
  );
}
