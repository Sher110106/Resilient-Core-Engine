import React from 'react';
import './ReceiverDashboard.css';

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
      <div className="receiver-connection">
        <label htmlFor="receiver-url">Receiver URL:</label>
        <input
          id="receiver-url"
          type="text"
          value={receiverUrl}
          onChange={(e) => onReceiverUrlChange(e.target.value)}
          placeholder="http://localhost:8080"
          className="receiver-url-input"
        />
        {status && status.listening && (
          <span className="status-indicator online">● Connected</span>
        )}
        {status && !status.listening && (
          <span className="status-indicator offline">● Offline</span>
        )}
      </div>

      {status && (
        <div className="receiver-stats">
          <div className="stat-card">
            <div className="stat-icon">🎯</div>
            <div className="stat-content">
              <div className="stat-value">{status.bind_addr || 'N/A'}</div>
              <div className="stat-label">Listening Address</div>
            </div>
          </div>

          <div className="stat-card">
            <div className="stat-icon">📦</div>
            <div className="stat-content">
              <div className="stat-value">{status.files_received || 0}</div>
              <div className="stat-label">Files Received</div>
            </div>
          </div>

          <div className="stat-card">
            <div className="stat-icon">💾</div>
            <div className="stat-content">
              <div className="stat-value">{formatBytes(status.total_size)}</div>
              <div className="stat-label">Total Size</div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default ReceiverDashboard;
