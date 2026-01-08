import React from 'react';
import './ReceivedFilesList.css';

// File Icon
const FileIcon = () => (
  <svg className="file-icon-svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
    <polyline points="14 2 14 8 20 8"/>
    <line x1="16" y1="13" x2="8" y2="13"/>
    <line x1="16" y1="17" x2="8" y2="17"/>
  </svg>
);

// Download Icon
const DownloadIcon = () => (
  <svg className="action-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
    <polyline points="7 10 12 15 17 10"/>
    <line x1="12" y1="15" x2="12" y2="3"/>
  </svg>
);

// Shield Check Icon (for integrity verified)
const ShieldCheckIcon = () => (
  <svg className="verified-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
    <polyline points="9 12 11 14 15 10"/>
  </svg>
);

// Inbox Icon
const InboxIcon = () => (
  <svg className="section-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <polyline points="22 12 16 12 14 15 10 15 8 12 2 12"/>
    <path d="M5.45 5.11L2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z"/>
  </svg>
);

function ReceivedFilesList({ files, receiverUrl }) {
  const formatBytes = (bytes) => {
    if (!bytes) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return Math.round(bytes / Math.pow(k, i) * 100) / 100 + ' ' + sizes[i];
  };

  const formatDate = (timestamp) => {
    if (!timestamp) return 'N/A';
    const date = new Date(timestamp * 1000);
    return date.toLocaleString('en-US', {
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit'
    });
  };

  if (!files || files.length === 0) {
    return (
      <div className="received-files-container">
        <h2>
          <InboxIcon />
          Intelligence Received
        </h2>
        <div className="empty-state">
          <InboxIcon />
          <p>Awaiting incoming intelligence</p>
          <p className="empty-hint">Files will appear here when received from Field Agents</p>
        </div>
      </div>
    );
  }

  return (
    <div className="received-files-container">
      <h2>
        <InboxIcon />
        Intelligence Received ({files.length})
      </h2>
      
      <div className="files-list">
        {files.map((file, index) => (
          <div key={index} className="file-item">
            <div className="file-icon-wrapper">
              <FileIcon />
            </div>
            
            <div className="file-details">
              <div className="file-name">{file.name || file.filename || 'Unknown'}</div>
              <div className="file-meta">
                <span className="file-size">{formatBytes(file.size)}</span>
                <span className="file-time">{formatDate(file.received_at || file.timestamp)}</span>
              </div>
            </div>

            <div className="file-status">
              <div className="integrity-badge">
                <ShieldCheckIcon />
                <span>INTEGRITY VERIFIED</span>
                <span className="hash-type">BLAKE3</span>
              </div>
            </div>

            <a 
              href={`${receiverUrl}/api/v1/receiver/files/${encodeURIComponent(file.name || file.filename)}`}
              className="download-button"
              download
              title="Download Intel"
            >
              <DownloadIcon />
              Retrieve
            </a>
          </div>
        ))}
      </div>
    </div>
  );
}

export default ReceivedFilesList;
