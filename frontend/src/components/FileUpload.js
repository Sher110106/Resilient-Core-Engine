import React, { useState } from 'react';
import './FileUpload.css';

// Upload Icon
const UploadIcon = () => (
  <svg className="upload-icon-svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
    <polyline points="17 8 12 3 7 8"/>
    <line x1="12" y1="3" x2="12" y2="15"/>
  </svg>
);

// File Icon
const FileIcon = () => (
  <svg className="file-icon-svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
    <polyline points="14 2 14 8 20 8"/>
    <line x1="16" y1="13" x2="8" y2="13"/>
    <line x1="16" y1="17" x2="8" y2="17"/>
    <polyline points="10 9 9 9 8 9"/>
  </svg>
);

// Radio Signal Icon
const SignalIcon = () => (
  <svg className="signal-icon-svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M5 12.55a11 11 0 0 1 14.08 0"/>
    <path d="M1.42 9a16 16 0 0 1 21.16 0"/>
    <path d="M8.53 16.11a6 6 0 0 1 6.95 0"/>
    <circle cx="12" cy="20" r="1"/>
  </svg>
);

function FileUpload({ onUpload }) {
  const [selectedFile, setSelectedFile] = useState(null);
  const [priority, setPriority] = useState('Normal');
  const [receiverAddr, setReceiverAddr] = useState('127.0.0.1:5001');
  const [dragActive, setDragActive] = useState(false);

  const handleDrag = (e) => {
    e.preventDefault();
    e.stopPropagation();
    if (e.type === "dragenter" || e.type === "dragover") {
      setDragActive(true);
    } else if (e.type === "dragleave") {
      setDragActive(false);
    }
  };

  const handleDrop = (e) => {
    e.preventDefault();
    e.stopPropagation();
    setDragActive(false);
    
    if (e.dataTransfer.files && e.dataTransfer.files[0]) {
      setSelectedFile(e.dataTransfer.files[0]);
    }
  };

  const handleChange = (e) => {
    e.preventDefault();
    if (e.target.files && e.target.files[0]) {
      setSelectedFile(e.target.files[0]);
    }
  };

  const handleSubmit = async (e) => {
    e.preventDefault();
    if (selectedFile) {
      await onUpload(selectedFile, priority, receiverAddr);
      setSelectedFile(null);
    }
  };

  const getPriorityClass = (p) => {
    if (p === 'Critical') return 'priority-critical';
    if (p === 'High') return 'priority-high';
    return 'priority-normal';
  };

  const getPriorityLabel = (p) => {
    if (p === 'Critical') return 'Life-Safety Data';
    if (p === 'High') return 'Damage Assessment';
    return 'Logistics/Supply';
  };

  return (
    <div className="file-upload-container">
      <h2>
        <SignalIcon />
        Transmit Critical Data
      </h2>
      
      <form onSubmit={handleSubmit} className="file-upload-form">
        <div 
          className={`drop-zone ${dragActive ? 'drag-active' : ''} ${selectedFile ? 'has-file' : ''}`}
          onDragEnter={handleDrag}
          onDragLeave={handleDrag}
          onDragOver={handleDrag}
          onDrop={handleDrop}
        >
          <input
            type="file"
            id="file-input"
            onChange={handleChange}
            className="file-input"
          />
          <label htmlFor="file-input" className="file-label">
            {selectedFile ? (
              <div className="file-info">
                <FileIcon />
                <span className="file-name">{selectedFile.name}</span>
                <span className="file-size">
                  {(selectedFile.size / 1024).toFixed(2)} KB
                </span>
                <span className={`priority-badge ${getPriorityClass(priority)}`}>
                  {getPriorityLabel(priority)}
                </span>
              </div>
            ) : (
              <div className="upload-prompt">
                <UploadIcon />
                <p className="upload-text">Drop mission-critical file here</p>
                <p className="upload-hint">or click to browse</p>
              </div>
            )}
          </label>
        </div>

        <div className="upload-options">
          <div className="priority-selector">
            <label htmlFor="priority">Priority Level:</label>
            <div className="priority-buttons">
              {['Critical', 'High', 'Normal'].map((p) => (
                <button
                  key={p}
                  type="button"
                  className={`priority-btn ${getPriorityClass(p)} ${priority === p ? 'active' : ''}`}
                  onClick={() => setPriority(p)}
                >
                  {p}
                </button>
              ))}
            </div>
          </div>

          <div className="receiver-address">
            <label htmlFor="receiver-addr">Command Center Address:</label>
            <input
              id="receiver-addr"
              type="text"
              value={receiverAddr}
              onChange={(e) => setReceiverAddr(e.target.value)}
              placeholder="127.0.0.1:5001"
              className="receiver-addr-input"
            />
          </div>
        </div>

        <button 
          type="submit" 
          className="upload-button"
          disabled={!selectedFile}
        >
          <SignalIcon />
          Initiate Secure Transmission
        </button>
      </form>
    </div>
  );
}

export default FileUpload;
