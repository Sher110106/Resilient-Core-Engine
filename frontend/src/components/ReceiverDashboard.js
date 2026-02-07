import React from 'react';
import { Radio, FileText, HardDrive } from 'lucide-react';
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
    <div className="receiver-dash">
      <div className="receiver-dash-header">
        <h3>Receiver</h3>
        <div className="receiver-status-indicator">
          {status && status.listening ? (
            <>
              <span className="dot online" />
              <span>Online</span>
            </>
          ) : (
            <>
              <span className="dot offline" />
              <span>Offline</span>
            </>
          )}
        </div>
      </div>

      <div className="receiver-endpoint">
        <label className="control-label">Endpoint</label>
        <input
          type="text"
          value={receiverUrl}
          onChange={(e) => onReceiverUrlChange(e.target.value)}
          placeholder="http://localhost:8080"
          className="endpoint-input"
        />
      </div>

      {status && (
        <div className="receiver-stats">
          <div className="receiver-stat">
            <Radio size={16} />
            <div>
              <span className="stat-val mono">{status.bind_addr || 'N/A'}</span>
              <span className="stat-lbl">Listening Address</span>
            </div>
          </div>
          <div className="receiver-stat">
            <FileText size={16} />
            <div>
              <span className="stat-val mono">{status.files_received || 0}</span>
              <span className="stat-lbl">Files Received</span>
            </div>
          </div>
          <div className="receiver-stat">
            <HardDrive size={16} />
            <div>
              <span className="stat-val mono">{formatBytes(status.total_size)}</span>
              <span className="stat-lbl">Total Data</span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default ReceiverDashboard;
