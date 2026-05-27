// The ReSide brand mark: a hex shell, a white Geist-800 "R" monogram, and a
// Dracula-purple arrow piercing in from the side (the "sideload" motion). This
// is the same artwork shipped as the app icon (src-tauri/icons), inlined here
// as SVG so the in-app logo stays crisp at any size and matches the launcher
// icon exactly. Geometry mirrors delivery/reside-icon/icon.svg.

export const ReSideMark = ({ size = 24, className = "" }: { size?: number; className?: string }) => (
  <svg
    viewBox="0 0 1024 1024"
    width={size}
    height={size}
    className={className}
    role="img"
    aria-label="ReSide"
  >
    <defs>
      <linearGradient id="reside-hex-bg" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" stopColor="#3a3548" />
        <stop offset="1" stopColor="#221f2c" />
      </linearGradient>
      <mask id="reside-slot" maskUnits="userSpaceOnUse" x="0" y="0" width="1024" height="1024">
        <rect width="1024" height="1024" fill="#fff" />
        <rect x="316" y="516" width="614" height="100" fill="#000" />
      </mask>
    </defs>
    <path
      d="M 512 64 L 932 296 L 932 760 L 512 992 L 92 760 L 92 296 Z"
      fill="url(#reside-hex-bg)"
      stroke="rgba(255,255,255,0.06)"
      strokeWidth="2"
    />
    <g mask="url(#reside-slot)">
      <text
        x="512"
        y="798"
        fontFamily="Geist, system-ui, sans-serif"
        fontWeight="800"
        fontSize="720"
        fill="#ffffff"
        textAnchor="middle"
        letterSpacing="-28"
      >
        R
      </text>
    </g>
    <rect x="446" y="516" width="484" height="100" fill="#bd93f9" />
    <path d="M 316 566 L 446 388 L 446 744 Z" fill="#bd93f9" />
  </svg>
);
