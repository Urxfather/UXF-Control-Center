import type { NavSectionId } from "../types/navigation";

interface IconProps {
  className?: string;
}

export function MenuIcon({ className = "h-5 w-5" }: IconProps) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2.4}
      strokeLinecap="butt"
      className={className}
      aria-hidden="true"
    >
      <path d="M4 6.5h16" />
      <path d="M4 12h16" />
      <path d="M4 17.5h16" />
    </svg>
  );
}

export function InfoIcon({ className = "h-4 w-4" }: IconProps) {
  return (
    <svg
      viewBox="0 0 20 20"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.5}
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      aria-hidden="true"
    >
      <path d="M5 3.5h6.5L15 7v9.5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1v-12a1 1 0 0 1 1-1Z" />
      <path d="M11.5 3.5V7H15" />
      <circle cx="9.5" cy="9.2" r="0.6" fill="currentColor" stroke="none" />
      <path d="M9.5 11.4v3.4" />
    </svg>
  );
}

export function MinimizeIcon({ className = "h-3.5 w-3.5" }: IconProps) {
  return (
    <svg
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.4}
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      aria-hidden="true"
    >
      <rect x="2.25" y="2.25" width="11.5" height="11.5" rx="1" />
      <path d="M4.75 10.75h4.25" />
    </svg>
  );
}

export function MaximizeIcon({ className = "h-3.5 w-3.5" }: IconProps) {
  return (
    <svg
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.4}
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      aria-hidden="true"
    >
      <path d="M5.5 2.5H2.5v3M10.5 2.5h3v3M2.5 10.5v3h3M13.5 10.5v3h-3" />
      <path d="M2.5 2.5l3.2 3.2M13.5 2.5l-3.2 3.2M2.5 13.5l3.2-3.2M13.5 13.5l-3.2-3.2" />
    </svg>
  );
}

export function RestoreIcon({ className = "h-3.5 w-3.5" }: IconProps) {
  return (
    <svg
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.4}
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      aria-hidden="true"
    >
      <rect x="2.5" y="5.2" width="8.3" height="8.3" rx="0.8" />
      <path d="M5.2 5.2V3.3a.8.8 0 0 1 .8-.8h6.7a.8.8 0 0 1 .8.8v6.7a.8.8 0 0 1-.8.8h-1.9" />
    </svg>
  );
}

export function CloseIcon({ className = "h-3.5 w-3.5" }: IconProps) {
  return (
    <svg
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.6}
      strokeLinecap="round"
      className={className}
      aria-hidden="true"
    >
      <path d="M3.5 3.5l9 9M12.5 3.5l-9 9" />
    </svg>
  );
}

function SystemIcon({ className = "h-7 w-7" }: IconProps) {
  return (
    <svg
      viewBox="0 0 32 32"
      fill="none"
      className={className}
      aria-hidden="true"
    >
      {/* Open laptop screen frame + tapered laptop deck base */}
      <path
        d="M5.5 10.5H9.2M22.8 10.5H26.5V22.5H5.5V10.5Z"
        stroke="currentColor"
        strokeWidth="1.7"
        strokeLinejoin="round"
      />
      <path
        d="M2.5 23.5H29.5L28 26.2H4L2.5 23.5Z"
        fill="currentColor"
        fillOpacity="0.18"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinejoin="round"
      />
      <rect x="13" y="23.5" width="6" height="1.2" fill="currentColor" />
      {/* Solid 8-tooth hardware gear overlapping top screen edge (hollow center) */}
      <path
        fillRule="evenodd"
        clipRule="evenodd"
        fill="currentColor"
        d="M14.7 4.2H17.3L17.9 6.2C18.4 6.4 18.8 6.6 19.3 6.9L21.1 5.9L22.9 7.7L21.9 9.5C22.2 10 22.4 10.4 22.6 10.9L24.6 11.5V14.1L22.6 14.7C22.4 15.2 22.2 15.6 21.9 16.1L22.9 17.9L21.1 19.7L19.3 18.7C18.8 19 18.4 19.2 17.9 19.4L17.3 21.4H14.7L14.1 19.4C13.6 19.2 13.2 19 12.7 18.7L10.9 19.7L9.1 17.9L10.1 16.1C9.8 15.6 9.6 15.2 9.4 14.7L7.4 14.1V11.5L9.4 10.9C9.6 10.4 9.8 10 10.1 9.5L9.1 7.7L10.9 5.9L12.7 6.9C13.2 6.6 13.6 6.4 14.1 6.2L14.7 4.2ZM16 15.6C17.55 15.6 18.8 14.35 18.8 12.8C18.8 11.25 17.55 10 16 10C14.45 10 13.2 11.25 13.2 12.8C13.2 14.35 14.45 15.6 16 15.6Z"
      />
    </svg>
  );
}

function KeyboardIcon({ className = "h-7 w-7" }: IconProps) {
  return (
    <svg
      viewBox="0 0 32 32"
      fill="none"
      className={className}
      aria-hidden="true"
    >
      {/* 5 upward illumination rays matching Acer LED Keyboard icon */}
      <path
        d="M8.5 9.2L7 6.2M12.2 8.2L11.4 4.8M16 7.8V4.2M19.8 8.2L20.6 4.8M23.5 9.2L25 6.2"
        stroke="currentColor"
        strokeWidth="1.7"
        strokeLinecap="round"
      />
      {/* Backlit chassis dome/rectangle */}
      <path
        d="M4.5 14.5C4.5 12.6 6 11.3 7.8 11.3H24.2C26 11.3 27.5 12.6 27.5 14.5V22.5C27.5 23.8 26.5 24.8 25.2 24.8H6.8C5.5 24.8 4.5 23.8 4.5 22.5V14.5Z"
        stroke="currentColor"
        strokeWidth="1.7"
        strokeLinejoin="round"
      />
      <path d="M5.2 14.8H26.8" stroke="currentColor" strokeWidth="1.2" strokeOpacity="0.7" />
      {/* Mini glowing keycap matrix inside */}
      <rect x="7" y="16.8" width="3.2" height="2.2" rx="0.4" fill="currentColor" />
      <rect x="11.5" y="16.8" width="3.2" height="2.2" rx="0.4" fill="currentColor" />
      <rect x="16" y="16.8" width="3.2" height="2.2" rx="0.4" fill="currentColor" />
      <rect x="20.5" y="16.8" width="4.5" height="2.2" rx="0.4" fill="currentColor" />
      <rect x="7" y="20.2" width="3.2" height="2.2" rx="0.4" fill="currentColor" />
      <rect x="11.5" y="20.2" width="9" height="2.2" rx="0.4" fill="currentColor" />
      <rect x="21.8" y="20.2" width="3.2" height="2.2" rx="0.4" fill="currentColor" />
    </svg>
  );
}

function FlexikeyIcon({ className = "h-7 w-7" }: IconProps) {
  return (
    <svg
      viewBox="0 0 32 32"
      fill="none"
      className={className}
      aria-hidden="true"
    >
      {/* Chamfered solid key tile with punched-out bold F matching reference */}
      <path
        fillRule="evenodd"
        clipRule="evenodd"
        fill="currentColor"
        d="M5.5 5.5H26.5V22.2L22.2 26.5H5.5V5.5ZM12.3 9.8V22.2H15.8V17.4H20.8V14.6H15.8V12.6H21.6V9.8H12.3Z"
      />
      {/* Subtle top-left corner triangle bevel cutout matching screenshot */}
      <path d="M6.8 6.8H10.5L6.8 10.5V6.8Z" fill="#13161d" fillOpacity="0.45" />
    </svg>
  );
}

function FlexiChargerIcon({ className = "h-7 w-7" }: IconProps) {
  return (
    <svg
      viewBox="0 0 32 32"
      fill="none"
      className={className}
      aria-hidden="true"
    >
      {/* Horizontal battery contour with solid left charge fill and protruding lightning bolt */}
      <path
        d="M4.5 11C4.5 10.2 5.2 9.5 6 9.5H24C24.8 9.5 25.5 10.2 25.5 11V13.1H27.2C27.8 13.1 28.2 13.6 28.2 14.2V17.8C28.2 18.4 27.8 18.9 27.2 18.9H25.5V21C25.5 21.8 24.8 22.5 24 22.5H6C5.2 22.5 4.5 21.8 4.5 21V11Z"
        stroke="currentColor"
        strokeWidth="1.6"
        strokeLinejoin="round"
      />
      {/* Left filled portion clipped around the lightning cutout */}
      <path
        d="M5.2 10.2H16.2L13.1 15.1H16.5L12.6 21.8H5.2V10.2Z"
        fill="currentColor"
      />
      {/* Bold angled lightning bolt extending cleanly above & below battery */}
      <path
        d="M17.4 6.2L11.2 16.5H15.6L13.6 25.8L21.5 14.8H16.8L17.4 6.2Z"
        fill="currentColor"
        stroke="#13161d"
        strokeWidth="1.1"
        strokeLinejoin="round"
      />
    </svg>
  );
}

export function NavSectionIcon({
  id,
  className,
}: {
  id: NavSectionId;
  className?: string;
}) {
  switch (id) {
    case "system":
      return <SystemIcon className={className} />;
    case "keyboard":
      return <KeyboardIcon className={className} />;
    case "flexikey":
      return <FlexikeyIcon className={className} />;
    case "flexicharger":
      return <FlexiChargerIcon className={className} />;
  }
}
