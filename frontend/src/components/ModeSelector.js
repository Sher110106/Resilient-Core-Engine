import React from 'react';
import { Send, Monitor } from 'lucide-react';
import './ModeSelector.css';

function ModeSelector({ mode, onModeChange }) {
  return (
    <div className="mode-selector">
      <button
        className={`mode-tab ${mode === 'sender' ? 'active' : ''}`}
        onClick={() => onModeChange('sender')}
      >
        <Send size={14} />
        Sender
      </button>
      <button
        className={`mode-tab ${mode === 'receiver' ? 'active' : ''}`}
        onClick={() => onModeChange('receiver')}
      >
        <Monitor size={14} />
        Receiver
      </button>
    </div>
  );
}

export default ModeSelector;
