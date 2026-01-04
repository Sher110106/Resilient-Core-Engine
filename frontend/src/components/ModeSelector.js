import React from 'react';
import './ModeSelector.css';

function ModeSelector({ mode, onModeChange }) {
  return (
    <div className="mode-selector">
      <button
        className={`mode-button ${mode === 'sender' ? 'active' : ''}`}
        onClick={() => onModeChange('sender')}
      >
        <span className="mode-icon">📤</span>
        <span className="mode-label">Sender</span>
      </button>
      <button
        className={`mode-button ${mode === 'receiver' ? 'active' : ''}`}
        onClick={() => onModeChange('receiver')}
      >
        <span className="mode-icon">📥</span>
        <span className="mode-label">Receiver</span>
      </button>
    </div>
  );
}

export default ModeSelector;
