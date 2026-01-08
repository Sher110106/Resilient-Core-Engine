import React from 'react';
import './ReceiverDashboard.css';

// Monitor Icon
const MonitorIcon = () => (
  <svg className="dashboard-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <rect x="2" y="3" width="20" height="14" rx="2" ry="2"/>
    <line x1="8" y1="21" x2="16" y2="21"/>
    <line x1="12" y1="17" x2="12" y2="21"/>
  </svg>
);

// Radio Icon
const RadioIcon = () => (
  <svg className="stat-icon-svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M4.9 19.1C1 15.2 1 8.8 4.9 4.9"/>
    <path d="M7.8 16.2c-2.3-2.3-2.3-6.1 0-8.5"/>
    <circle cx="12" cy="12" r="2"/>
    <path d="M16.2 7.8c2.3 2.3 2.3 6.1 0 8.5"/>
    <path d="M19.1 4.9C23 8.8 23 15.1 19.1 19"/>
  </svg>
);

// File Icon
const FileIcon = () => (
  <svg className="stat-icon-svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
    <polyline points="14 2 14 8 20 8"/>
  </svg>
);

// Database Icon
const DatabaseIcon = () => (
  <svg className="stat-icon-svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <ellipse cx="12" cy="5" rx="9" ry="3"/>
    <path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"/>
    <path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/>
  </svg>
);

function ReceiverDashboard({ status, receiverUrl, onReceiverUrlChange }) {
  const formatBytes = (bytes) => {
    if (!bytes) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return Math.round(bytes / Math.pow(k, i) * 100) / 100 + ' ' + sizes[i];
  };

  return (
    <div className="receiver-dashboard">
      <div className="dashboard-header">
        <h2>
          <MonitorIcon />
          Command Center
        </h2>
        <div className="connection-status">
          {status && status.listening ? (
            <span className="status-indicator online">
              <span className="status-dot"></span>
              Signal Active
            </span>
          ) : (
            <span className="status-indicator offline">
              <span className="status-dot"></span>
              No Signal
            </span>
          )}
        </div>
      </div>

      <div className="receiver-connection">
        <label htmlFor="receiver-url">Receiver Endpoint:</label>
        <input
          id="receiver-url"
          type="text"
          value={receiverUrl}
          onChange={(e) => onReceiverUrlChange(e.target.value)}
          placeholder="http://localhost:8080"
          className="receiver-url-input"
        />
      </div>

      {status && (
        <div className="receiver-stats">
          <div className="stat-card">
            <div className="stat-icon-wrapper">
              <RadioIcon />
            </div>
            <div className="stat-content">
              <div className="stat-value">{status.bind_addr || 'N/A'}</div>
              <div className="stat-label">Listening Address</div>
            </div>
          </div>

          <div className="stat-card">
            <div className="stat-icon-wrapper">
              <FileIcon />
            </div>
            <div className="stat-content">
              <div className="stat-value">{status.files_received || 0}</div>
              <div className="stat-label">Intel Received</div>
            </div>
          </div>

          <div className="stat-card">
            <div className="stat-icon-wrapper">
              <DatabaseIcon />
            </div>
            <div className="stat-content">
              <div className="stat-value">{formatBytes(status.total_size)}</div>
              <div className="stat-label">Total Data</div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default ReceiverDashboard;
