import React from 'react';
import './ModeSelector.css';

// Field Agent Icon (person with radio)
const FieldAgentIcon = () => (
  <svg className="mode-icon-svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <circle cx="12" cy="7" r="4"/>
    <path d="M5.5 21v-2a6.5 6.5 0 0 1 13 0v2"/>
    <path d="M17 12l2-2"/>
    <path d="M19 10l2 2"/>
    <path d="M21 8v4"/>
  </svg>
);

// Command Center Icon (screens/dashboard)
const CommandCenterIcon = () => (
  <svg className="mode-icon-svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <rect x="2" y="3" width="20" height="14" rx="2" ry="2"/>
    <line x1="8" y1="21" x2="16" y2="21"/>
    <line x1="12" y1="17" x2="12" y2="21"/>
    <line x1="6" y1="8" x2="10" y2="8"/>
    <line x1="6" y1="11" x2="18" y2="11"/>
    <circle cx="16" cy="8" r="1"/>
  </svg>
);

function ModeSelector({ mode, onModeChange }) {
  return (
    <div className="mode-selector">
      <button
        className={`mode-button ${mode === 'sender' ? 'active' : ''}`}
        onClick={() => onModeChange('sender')}
      >
        <FieldAgentIcon />
        <span className="mode-label">Field Agent</span>
        <span className="mode-desc">Transmit Data</span>
      </button>
      <button
        className={`mode-button ${mode === 'receiver' ? 'active' : ''}`}
        onClick={() => onModeChange('receiver')}
      >
        <CommandCenterIcon />
        <span className="mode-label">Command Center</span>
        <span className="mode-desc">Receive Intel</span>
      </button>
    </div>
  );
}

export default ModeSelector;
