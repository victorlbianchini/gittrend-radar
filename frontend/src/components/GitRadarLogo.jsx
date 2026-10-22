import React from 'react';

/**
 * GitRadarLogo - Futuristic Git Branch & Radar Emblem
 * Merges Git branching topology (nodes, edges, commit branches)
 * with an atmospheric concentric radar scanner and cyberpunk neon glow.
 */
export default function GitRadarLogo({ size = 46, animated = true }) {
  const s = size;

  return (
    <div 
      className={`git-radar-logo-container ${animated ? 'is-animated' : ''}`}
      style={{
        width: s,
        height: s,
        position: 'relative',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        flexShrink: 0
      }}
    >
      {/* Outer ambient glow halo */}
      <div 
        className="git-radar-halo"
        style={{
          position: 'absolute',
          inset: -3,
          borderRadius: Math.round(s * 0.32),
          background: 'linear-gradient(135deg, rgba(6, 182, 212, 0.4), rgba(99, 102, 241, 0.45), rgba(16, 185, 129, 0.35))',
          filter: 'blur(8px)',
          opacity: 0.75,
          zIndex: 0
        }}
      />

      {/* Main glass badge container */}
      <svg 
        width={s} 
        height={s} 
        viewBox="0 0 64 64" 
        fill="none" 
        xmlns="http://www.w3.org/2000/svg"
        style={{
          position: 'relative',
          zIndex: 1,
          filter: 'drop-shadow(0 4px 12px rgba(6, 182, 212, 0.35))'
        }}
      >
        <defs>
          {/* Badge Background Gradients */}
          <linearGradient id="badgeBg" x1="0" y1="0" x2="64" y2="64" gradientUnits="userSpaceOnUse">
            <stop offset="0%" stopColor="#0b1329" />
            <stop offset="50%" stopColor="#080e1e" />
            <stop offset="100%" stopColor="#030712" />
          </linearGradient>

          <linearGradient id="badgeBorder" x1="0" y1="0" x2="64" y2="64" gradientUnits="userSpaceOnUse">
            <stop offset="0%" stopColor="#38bdf8" stopOpacity="0.8" />
            <stop offset="40%" stopColor="#818cf8" stopOpacity="0.6" />
            <stop offset="100%" stopColor="#10b981" stopOpacity="0.4" />
          </linearGradient>

          {/* Git Branch Lines Gradient */}
          <linearGradient id="branchMain" x1="20" y1="52" x2="20" y2="14" gradientUnits="userSpaceOnUse">
            <stop offset="0%" stopColor="#06b6d4" />
            <stop offset="60%" stopColor="#6366f1" />
            <stop offset="100%" stopColor="#a855f7" />
          </linearGradient>

          <linearGradient id="branchFork" x1="20" y1="36" x2="44" y2="22" gradientUnits="userSpaceOnUse">
            <stop offset="0%" stopColor="#6366f1" />
            <stop offset="50%" stopColor="#22d3ee" />
            <stop offset="100%" stopColor="#10b981" />
          </linearGradient>

          {/* Radar Sweep Gradient */}
          <radialGradient id="radarSweep" cx="32" cy="32" r="32" gradientUnits="userSpaceOnUse">
            <stop offset="0%" stopColor="#38bdf8" stopOpacity="0.25" />
            <stop offset="70%" stopColor="#06b6d4" stopOpacity="0.08" />
            <stop offset="100%" stopColor="#06b6d4" stopOpacity="0" />
          </radialGradient>
        </defs>

        {/* Badge Base Plate */}
        <rect 
          x="1" 
          y="1" 
          width="62" 
          height="62" 
          rx="18" 
          fill="url(#badgeBg)" 
          stroke="url(#badgeBorder)" 
          strokeWidth="1.5" 
        />

        {/* Concentric Radar Rings */}
        <circle cx="32" cy="32" r="24" stroke="rgba(56, 189, 248, 0.12)" strokeWidth="1" strokeDasharray="3 3" />
        <circle cx="32" cy="32" r="16" stroke="rgba(99, 102, 241, 0.18)" strokeWidth="1" />
        <circle cx="32" cy="32" r="8" stroke="rgba(16, 185, 129, 0.22)" strokeWidth="1" strokeDasharray="2 2" />

        {/* Radar Center Coordinate Marks */}
        <line x1="32" y1="8" x2="32" y2="12" stroke="rgba(56, 189, 248, 0.4)" strokeWidth="1.2" strokeLinecap="round" />
        <line x1="32" y1="52" x2="32" y2="56" stroke="rgba(56, 189, 248, 0.4)" strokeWidth="1.2" strokeLinecap="round" />
        <line x1="8" y1="32" x2="12" y2="32" stroke="rgba(56, 189, 248, 0.4)" strokeWidth="1.2" strokeLinecap="round" />
        <line x1="52" y1="32" x2="56" y2="32" stroke="rgba(56, 189, 248, 0.4)" strokeWidth="1.2" strokeLinecap="round" />

        {/* Radar Dynamic Scan Beam Overlay */}
        <path 
          className="radar-beam"
          d="M 32 32 L 56 32 A 24 24 0 0 0 49 15 Z" 
          fill="url(#radarSweep)" 
        />

        {/* ==================== GIT TOPOLOGY ==================== */}

        {/* Main Trunk Path (Vertical Trunk) */}
        <path 
          d="M 22 48 L 22 18" 
          stroke="url(#branchMain)" 
          strokeWidth="3.2" 
          strokeLinecap="round" 
        />

        {/* Forked Branch Curve (Bezier Path to right branch) */}
        <path 
          d="M 22 36 C 22 28, 30 22, 42 22" 
          stroke="url(#branchFork)" 
          strokeWidth="3" 
          strokeLinecap="round" 
          fill="none" 
        />

        {/* Git Nodes (Commit circles with glows) */}

        {/* 1. Root Node (Bottom main) */}
        <circle cx="22" cy="48" r="4.5" fill="#06b6d4" />
        <circle cx="22" cy="48" r="2.2" fill="#ffffff" />

        {/* 2. Middle Fork Pivot Node */}
        <circle cx="22" cy="36" r="3.8" fill="#6366f1" />
        <circle cx="22" cy="36" r="1.8" fill="#ffffff" />

        {/* 3. Main Head Node (Top main) */}
        <circle cx="22" cy="18" r="4.5" fill="#a855f7" />
        <circle cx="22" cy="18" r="2.2" fill="#ffffff" />

        {/* 4. Branch Head Node (Right branch tip) */}
        <circle cx="42" cy="22" r="5" fill="#10b981" />
        <circle cx="42" cy="22" r="2.5" fill="#ffffff" />

        {/* Pulsing Signal Dot around Branch Head */}
        <circle 
          className="signal-pulse" 
          cx="42" 
          cy="22" 
          r="8" 
          stroke="#34d399" 
          strokeWidth="1.2" 
          fill="none" 
          opacity="0.6" 
        />
      </svg>
    </div>
  );
}
