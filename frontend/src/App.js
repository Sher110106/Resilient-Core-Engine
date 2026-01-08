import React, { useState, useEffect, useRef } from 'react';
import './App.css';
import FileUpload from './components/FileUpload';
import TransferList from './components/TransferList';
import Dashboard from './components/Dashboard';
import ModeSelector from './components/ModeSelector';
import ReceiverDashboard from './components/ReceiverDashboard';
import ReceivedFilesList from './components/ReceivedFilesList';
import api from './services/api';
import receiverApi from './services/receiverApi';

// Radio Tower Icon SVG
const RadioTowerIcon = () => (
  <svg className="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M4.9 19.1C1 15.2 1 8.8 4.9 4.9"/>
    <path d="M7.8 16.2c-2.3-2.3-2.3-6.1 0-8.5"/>
    <circle cx="12" cy="12" r="2"/>
    <path d="M16.2 7.8c2.3 2.3 2.3 6.1 0 8.5"/>
    <path d="M19.1 4.9C23 8.8 23 15.1 19.1 19"/>
    <line x1="12" y1="14" x2="12" y2="22"/>
  </svg>
);

function App() {
  const [mode, setMode] = useState('sender'); // 'sender' (Field Agent) or 'receiver' (Command Center)
  const [transfers, setTransfers] = useState([]);
  const [stats, setStats] = useState({ active: 0, completed: 0, failed: 0 });
  const wsRef = useRef(null);
  
  // Receiver state
  const [receiverUrl, setReceiverUrl] = useState('http://localhost:8080');
  const [receiverStatus, setReceiverStatus] = useState(null);
  const [receivedFiles, setReceivedFiles] = useState([]);

  useEffect(() => {
    if (mode === 'sender') {
      // Load initial transfers
      loadTransfers();

      // Setup WebSocket connection
      setupWebSocket();

      // Poll for updates every 2 seconds as backup
      const interval = setInterval(loadTransfers, 2000);

      return () => {
        clearInterval(interval);
        if (wsRef.current) {
          wsRef.current.close();
        }
      };
    } else {
      // Receiver mode: load receiver data
      loadReceiverData();
      
      // Poll receiver every 2 seconds
      const interval = setInterval(loadReceiverData, 2000);
      
      return () => {
        clearInterval(interval);
      };
    }
  }, [mode, receiverUrl]);

  const setupWebSocket = () => {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${protocol}//${window.location.hostname}:3000/ws`;
    
    try {
      const ws = new WebSocket(wsUrl);
      
      ws.onopen = () => {
        console.log('WebSocket connected');
      };
      
      ws.onmessage = (event) => {
        try {
          const message = JSON.parse(event.data);
          handleWebSocketMessage(message);
        } catch (error) {
          console.error('Error parsing WebSocket message:', error);
        }
      };
      
      ws.onerror = (error) => {
        console.error('WebSocket error:', error);
      };
      
      ws.onclose = () => {
        console.log('WebSocket disconnected, reconnecting in 3s...');
        setTimeout(setupWebSocket, 3000);
      };
      
      wsRef.current = ws;
    } catch (error) {
      console.error('Error creating WebSocket:', error);
    }
  };

  const handleWebSocketMessage = (message) => {
    if (message.type === 'TransferProgress' && message.data) {
      setTransfers(prev => {
        const index = prev.findIndex(t => t.session_id === message.data.session_id);
        if (index >= 0) {
          const updated = [...prev];
          updated[index] = { ...updated[index], ...message.data };
          return updated;
        }
        return prev;
      });
    }
  };

  const loadTransfers = async () => {
    try {
      const response = await api.listTransfers();
      if (response.active_transfers && response.active_transfers.length > 0) {
        // Load details for each transfer
        const detailsPromises = response.active_transfers.map(id => 
          api.getProgress(id).catch(() => null)
        );
        const details = await Promise.all(detailsPromises);
        setTransfers(details.filter(d => d !== null));
        
        // Update stats
        const active = details.filter(d => d && d.status === 'Active').length;
        const completed = details.filter(d => d && d.status === 'Completed').length;
        const failed = details.filter(d => d && d.status?.Failed).length;
        setStats({ active, completed, failed });
      } else {
        setTransfers([]);
        setStats({ active: 0, completed: 0, failed: 0 });
      }
    } catch (error) {
      console.error('Error loading transfers:', error);
    }
  };

  const handleFileUpload = async (file, priority, receiverAddr) => {
    try {
      const result = await api.uploadAndTransfer(file, priority, receiverAddr);
      console.log('Transfer started:', result);
      alert(`Mission initiated! Session ID: ${result.session_id}`);
      // Reload transfers to show the new one
      setTimeout(loadTransfers, 500);
    } catch (error) {
      console.error('Error starting transfer:', error);
      alert(`Failed to initiate mission: ${error.response?.data?.error || error.message}`);
    }
  };

  const handlePause = async (sessionId) => {
    try {
      await api.pauseTransfer(sessionId);
      loadTransfers();
    } catch (error) {
      console.error('Error pausing transfer:', error);
    }
  };

  const handleResume = async (sessionId) => {
    try {
      await api.resumeTransfer(sessionId);
      loadTransfers();
    } catch (error) {
      console.error('Error resuming transfer:', error);
    }
  };

  const handleCancel = async (sessionId) => {
    try {
      await api.cancelTransfer(sessionId);
      loadTransfers();
    } catch (error) {
      console.error('Error cancelling transfer:', error);
    }
  };

  const loadReceiverData = async () => {
    try {
      receiverApi.setBaseUrl(receiverUrl);
      const [status, files] = await Promise.all([
        receiverApi.getStatus(),
        receiverApi.listFiles()
      ]);
      setReceiverStatus(status);
      setReceivedFiles(files);
    } catch (error) {
      console.error('Error loading receiver data:', error);
      setReceiverStatus({ listening: false });
      setReceivedFiles([]);
    }
  };

  const handleModeChange = (newMode) => {
    setMode(newMode);
  };

  const handleReceiverUrlChange = (url) => {
    setReceiverUrl(url);
  };

  return (
    <div className="App">
      <header className="App-header">
        <h1>
          <RadioTowerIcon />
          RESILIENT
        </h1>
        <p>Disaster Data Link — Resilient Transmission for Critical Missions</p>
      </header>

      <ModeSelector mode={mode} onModeChange={handleModeChange} />

      <main className="App-main">
        {mode === 'sender' ? (
          <>
            <Dashboard stats={stats} />
            <FileUpload onUpload={handleFileUpload} />
            <TransferList 
              transfers={transfers}
              onPause={handlePause}
              onResume={handleResume}
              onCancel={handleCancel}
            />
          </>
        ) : (
          <>
            <ReceiverDashboard 
              status={receiverStatus}
              receiverUrl={receiverUrl}
              onReceiverUrlChange={handleReceiverUrlChange}
            />
            <ReceivedFilesList 
              files={receivedFiles}
              receiverUrl={receiverUrl}
            />
          </>
        )}
      </main>

      <footer className="App-footer">
        <p>RESILIENT v1.0.0 — Powered by QUIC Protocol with Erasure Coding</p>
      </footer>
    </div>
  );
}

export default App;
