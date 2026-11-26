export function SuperLockIcon({ active = false }: { active?: boolean }) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      className="h-10 w-10"
      style={active ? { filter: "drop-shadow(0 0 8px rgba(155,224,0,0.45))" } : undefined}
    >
      <rect
        x="3"
        y="3.5"
        width="16"
        height="15.5"
        rx="2.6"
        fill={active ? "#9be000" : "#181e27"}
        stroke={active ? "#9be000" : "currentColor"}
        strokeWidth="1.4"
      />
      <text
        x="11"
        y="13.2"
        fontSize="5.8"
        fill={active ? "#0a0c10" : "currentColor"}
        stroke="none"
        textAnchor="middle"
        fontWeight="800"
        letterSpacing="0.3"
      >
        SUP
      </text>
      <rect
        x="14.5"
        y="14"
        width="6.5"
        height="5.8"
        rx="1.1"
        fill="#0b0e13"
        stroke={active ? "#9be000" : "currentColor"}
        strokeWidth="1.4"
      />
      <path
        d="M16.2 14v-1.4a1.55 1.55 0 0 1 3.1 0V14"
        stroke={active ? "#9be000" : "currentColor"}
        strokeWidth="1.4"
      />
    </svg>
  );
}

export function SuperFnSwapIcon({ active = false }: { active?: boolean }) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.45"
      className="h-10 w-10"
      style={active ? { filter: "drop-shadow(0 0 8px rgba(155,224,0,0.45))" } : undefined}
    >
      <rect
        x="2.8"
        y="3.5"
        width="9.5"
        height="7.5"
        rx="1.3"
        fill={active ? "#9be000" : "none"}
        stroke={active ? "#9be000" : "currentColor"}
      />
      <text
        x="7.55"
        y="8.7"
        fontSize="4.1"
        fill={active ? "#0a0c10" : "currentColor"}
        stroke="none"
        textAnchor="middle"
        fontWeight="700"
      >
        SUP
      </text>
      <rect
        x="11.7"
        y="13"
        width="9.5"
        height="7.5"
        rx="1.3"
        fill={active ? "#9be000" : "none"}
        stroke={active ? "#9be000" : "currentColor"}
      />
      <text
        x="16.45"
        y="18.2"
        fontSize="5.4"
        fill={active ? "#0a0c10" : "currentColor"}
        stroke="none"
        textAnchor="middle"
        fontWeight="700"
      >
        fn
      </text>
      <path
        d="M15 6.2h3.8v3.2M18.8 6.2L16.3 8.7M9 17.8H5.2v-3.2M5.2 17.8l2.5-2.5"
        stroke={active ? "#9be000" : "currentColor"}
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

export function FnLockIcon({ active = false }: { active?: boolean }) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      className="h-10 w-10"
      style={active ? { filter: "drop-shadow(0 0 8px rgba(155,224,0,0.45))" } : undefined}
    >
      <rect
        x="3.5"
        y="3.5"
        width="17"
        height="17"
        rx="3"
        fill={active ? "#9be000" : "none"}
        stroke={active ? "#9be000" : "currentColor"}
      />
      <text
        x="12"
        y="14.8"
        fontSize="10"
        fill={active ? "#0a0c10" : "currentColor"}
        stroke="none"
        textAnchor="middle"
        fontWeight="700"
      >
        fn
      </text>
    </svg>
  );
}

export function CameraIcon({ active = false }: { active?: boolean }) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      className="h-10 w-10"
      style={active ? { filter: "drop-shadow(0 0 8px rgba(155,224,0,0.45))" } : undefined}
    >
      <circle
        cx="12"
        cy="10"
        r="4.5"
        fill={active ? "#9be000" : "none"}
        fillOpacity={active ? "0.22" : "0"}
        stroke={active ? "#9be000" : "currentColor"}
      />
      <circle
        cx="12"
        cy="10"
        r="1.8"
        fill={active ? "#9be000" : "none"}
        stroke={active ? "#9be000" : "currentColor"}
      />
      <path
        d="M7.5 18.5h9l-1.5-2.5H9l-1.5 2.5Z"
        stroke={active ? "#9be000" : "currentColor"}
        fill={active ? "#9be000" : "none"}
        fillOpacity={active ? "0.35" : "0"}
        strokeLinejoin="round"
      />
      <path
        d="M5 10a7 7 0 0 1 14 0"
        stroke={active ? "#9be000" : "currentColor"}
        strokeDasharray="2 2"
      />
    </svg>
  );
}

export function TouchpadIcon({ active = false }: { active?: boolean }) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      className="h-10 w-10"
      style={active ? { filter: "drop-shadow(0 0 8px rgba(155,224,0,0.45))" } : undefined}
    >
      <rect
        x="3"
        y="4.5"
        width="18"
        height="15"
        rx="2"
        fill={active ? "#9be000" : "none"}
        fillOpacity={active ? "0.22" : "0"}
        stroke={active ? "#9be000" : "currentColor"}
      />
      {active && (
        <rect x="3.75" y="5.25" width="16.5" height="9.25" rx="1.2" fill="#9be000" stroke="none" />
      )}
      <path
        d="M3 14.8h18M12 14.8V19.5"
        stroke={active ? "#9be000" : "currentColor"}
      />
    </svg>
  );
}

export function AirplaneIcon({ active = false }: { active?: boolean }) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill={active ? "#9be000" : "none"}
      fillOpacity={active ? "0.28" : "0"}
      stroke={active ? "#9be000" : "currentColor"}
      strokeWidth="1.5"
      className="h-10 w-10"
      style={active ? { filter: "drop-shadow(0 0 8px rgba(155,224,0,0.45))" } : undefined}
    >
      <path d="M12 3.5l2 6 6.5 3.5v1.8l-6.5-1.6v4.2l2 1.6v1.4l-4-1-4 1v-1.4l2-1.6v-4.2l-6.5 1.6V13L10 9.5l2-6Z" strokeLinejoin="round" />
    </svg>
  );
}

export function KeycapBadgeIcon({
  text,
  active = false,
}: {
  text: string;
  active?: boolean;
}) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      className="h-10 w-10"
      style={active ? { filter: "drop-shadow(0 0 8px rgba(155,224,0,0.45))" } : undefined}
    >
      <rect
        x="3"
        y="3.5"
        width="18"
        height="17"
        rx="3.2"
        fill={active ? "#9be000" : "#181e27"}
        stroke={active ? "#9be000" : "#4b5563"}
        strokeWidth="1.35"
      />
      <text
        x="12"
        y="14.2"
        fontSize={text.length > 3 ? "6.6" : "7.4"}
        fill={active ? "#0a0c10" : "#7c8594"}
        textAnchor="middle"
        fontWeight="700"
      >
        {text}
      </text>
    </svg>
  );
}

export function BlueLightIcon() {
  return (
    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" className="h-4 w-4">
      <circle cx="8" cy="8" r="3" />
      <path d="M8 2v1.5M8 12.5V14M2 8h1.5M12.5 8H14M3.8 3.8l1 1M11.2 11.2l1 1M12.2 3.8l-1 1M4.8 11.2l-1 1" strokeLinecap="round" />
    </svg>
  );
}

export function BrightnessIcon() {
  return (
    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" className="h-4 w-4">
      <circle cx="8" cy="8" r="2.6" fill="currentColor" />
      <path d="M8 1.8v1.6M8 12.6v1.6M1.8 8h1.6M12.6 8h1.6M3.6 3.6l1.1 1.1M11.3 11.3l1.1 1.1M12.4 3.6l-1.1 1.1M4.7 11.3l-1.1 1.1" strokeLinecap="round" />
    </svg>
  );
}

export function VolumeIcon() {
  return (
    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" className="h-4 w-4">
      <path d="M3 6.2h2.6L9 3.5v9L5.6 9.8H3V6.2Z" strokeLinejoin="round" />
      <path d="M11.3 5.5a3.5 3.5 0 0 1 0 5M13 3.8a6 6 0 0 1 0 8.4" strokeLinecap="round" />
    </svg>
  );
}
