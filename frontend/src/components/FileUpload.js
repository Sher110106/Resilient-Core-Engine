import React, { useState } from 'react';
import { Upload, File, Send } from 'lucide-react';
import './FileUpload.css';

function FileUpload({ onUpload }) {
  const [selectedFile, setSelectedFile] = useState(null);
  const [priority, setPriority] = useState('Normal');
  const [receiverAddr, setReceiverAddr] = useState('127.0.0.1:5001');
  const [dragActive, setDragActive] = useState(false);
  const [sending, setSending] = useState(false);

  const handleDrag = (e) => {
    e.preventDefault();
    e.stopPropagation();
    if (e.type === 'dragenter' || e.type === 'dragover') {
      setDragActive(true);
    } else if (e.type === 'dragleave') {
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
    if (selectedFile && !sending) {
      setSending(true);
      try {
        await onUpload(selectedFile, priority, receiverAddr);
        setSelectedFile(null);
      } catch (err) {
        // handled upstream
      }
      setSending(false);
    }
  };

  const formatSize = (bytes) => {
    if (bytes < 1024) return bytes + ' B';
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
    return (bytes / (1024 * 1024)).toFixed(2) + ' MB';
  };

  return (
    <div className="file-upload">
      <div className="section-header">
        <h3>Transfer File</h3>
      </div>

      <form onSubmit={handleSubmit} className="upload-form">
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
            className="file-input-hidden"
          />
          <label htmlFor="file-input" className="drop-label">
            {selectedFile ? (
              <div className="file-selected">
                <File size={20} />
                <span className="file-name">{selectedFile.name}</span>
                <span className="file-size">{formatSize(selectedFile.size)}</span>
              </div>
            ) : (
              <div className="drop-prompt">
                <Upload size={24} />
                <span>Drop file here or click to browse</span>
              </div>
            )}
          </label>
        </div>

        <div className="upload-controls">
          <div className="control-group">
            <label className="control-label">Priority</label>
            <div className="priority-pills">
              {['Critical', 'High', 'Normal'].map((p) => (
                <button
                  key={p}
                  type="button"
                  className={`priority-pill ${priority === p ? 'active' : ''} ${p.toLowerCase()}`}
                  onClick={() => setPriority(p)}
                >
                  {p}
                </button>
              ))}
            </div>
          </div>

          <div className="control-group">
            <label className="control-label">Receiver Address</label>
            <input
              type="text"
              value={receiverAddr}
              onChange={(e) => setReceiverAddr(e.target.value)}
              placeholder="127.0.0.1:5001"
              className="addr-input"
            />
          </div>
        </div>

        <button
          type="submit"
          className="send-button"
          disabled={!selectedFile || sending}
        >
          <Send size={16} />
          {sending ? 'Sending...' : 'Send'}
        </button>
      </form>
    </div>
  );
}

export default FileUpload;
