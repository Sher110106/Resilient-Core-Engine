import React, { useState, useEffect, useRef, useCallback } from 'react';
import { Radio } from 'lucide-react';
import './App.css';
import FileUpload from './components/FileUpload';
import TransferList from './components/TransferList';
import Dashboard from './components/Dashboard';
import ModeSelector from './components/ModeSelector';
import ReceiverDashboard from './components/ReceiverDashboard';
import ReceivedFilesList from './components/ReceivedFilesList';
import MetricsPanel from './components/MetricsPanel';
import PacketLossSimulator from './components/PacketLossSimulator';
import ComparisonView from './components/ComparisonView';
import AnimatedDataFlow from './components/AnimatedDataFlow';
import api from './services/api';
import receiverApi from './services/receiverApi';

function App() {
  const [mode, setMode] = useState('sender');
  const [transfers, setTransfers] = useState([]);
  const [stats, setStats] = useState({ active: 0, completed: 0, failed: 0 });
  const [metricsHistory, setMetricsHistory] = useState([]);
  const [currentMetrics, setCurrentMetrics] = useState(null);
  const [connected, setConnected] = useState(false);
  const wsRef = useRef(null);

  // Track the last uploaded file for simulation
  const [uploadedFilePath, setUploadedFilePath] = useState(null);
  const [uploadedFileName, setUploadedFileName] = useState(null);

  // Receiver state
  const [receiverUrl, setReceiverUrl] = useState('http://localhost:8080');
  const [receiverStatus, setReceiverStatus] = useState(null);
  const [receivedFiles, setReceivedFiles] = useState([]);

  const handleWebSocketMessage = useCallback((message) => {
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
    if (message.type === 'MetricsSnapshot' && message.data) {
      setCurrentMetrics(message.data);
      setMetricsHistory(prev => {
        const next = [...prev, { ...message.data, time: new Date().toLocaleTimeString() }];
        if (next.length > 120) next.shift();
        return next;
      });
    }
  }, []);

  const setupWebSocket = useCallback(() => {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${protocol}//${window.location.hostname}:3000/ws`;

    try {
      const ws = new WebSocket(wsUrl);

      ws.onopen = () => {
        setConnected(true);
      };

      ws.onmessage = (event) => {
        try {
          const message = JSON.parse(event.data);
          handleWebSocketMessage(message);
        } catch (error) {
          // ignore parse errors
        }
      };

      ws.onerror = () => {
        setConnected(false);
      };

      ws.onclose = () => {
        setConnected(false);
        setTimeout(setupWebSocket, 3000);
      };

      wsRef.current = ws;
    } catch (error) {
      setConnected(false);
    }
  }, [handleWebSocketMessage]);

  const loadTransfers = useCallback(async () => {
    try {
      const response = await api.listTransfers();
      if (response.active_transfers && response.active_transfers.length > 0) {
        const detailsPromises = response.active_transfers.map(id =>
          api.getProgress(id).catch(() => null)
        );
        const details = await Promise.all(detailsPromises);
        setTransfers(details.filter(d => d !== null));

        const active = details.filter(d => d && d.status === 'Active').length;
        const completed = details.filter(d => d && d.status === 'Completed').length;
        const failed = details.filter(d => d && d.status?.Failed).length;
        setStats({ active, completed, failed });
      } else {
        setTransfers([]);
        setStats({ active: 0, completed: 0, failed: 0 });
      }
    } catch (error) {
      // silent
    }
  }, []);

  const loadReceiverData = useCallback(async () => {
    try {
      receiverApi.setBaseUrl(receiverUrl);
      const [status, files] = await Promise.all([
        receiverApi.getStatus(),
        receiverApi.listFiles()
      ]);
      setReceiverStatus(status);
      setReceivedFiles(files);
    } catch (error) {
      setReceiverStatus({ listening: false });
      setReceivedFiles([]);
    }
  }, [receiverUrl]);

  useEffect(() => {
    if (mode === 'sender') {
      loadTransfers();
      setupWebSocket();
      const interval = setInterval(loadTransfers, 2000);
      return () => {
        clearInterval(interval);
        if (wsRef.current) {
          wsRef.current.close();
        }
      };
    } else {
      loadReceiverData();
      const interval = setInterval(loadReceiverData, 2000);
      return () => {
        clearInterval(interval);
      };
    }
  }, [mode, loadTransfers, loadReceiverData, setupWebSocket]);

  // Load previously uploaded files on mount so simulation works across refreshes
  useEffect(() => {
    if (mode === 'sender' && !uploadedFilePath) {
      api.listUploads().then(data => {
        if (data.files && data.files.length > 0) {
          // Pick the last file (most recently listed)
          const latest = data.files[data.files.length - 1];
          setUploadedFilePath(latest.file_path);
          setUploadedFileName(latest.file_name);
        }
      }).catch(() => {
        // silent - uploads endpoint might not be available yet
      });
    }
  }, [mode, uploadedFilePath]);

  const handleFileUpload = async (file, priority, receiverAddr) => {
    try {
      const result = await api.uploadAndTransfer(file, priority, receiverAddr);
      // Track the uploaded file path for simulation
      if (result.file_path) {
        setUploadedFilePath(result.file_path);
        setUploadedFileName(result.file_name || file.name);
      }
      setTimeout(loadTransfers, 500);
      return result;
    } catch (error) {
      throw error;
    }
  };

  const handlePause = async (sessionId) => {
    try {
      await api.pauseTransfer(sessionId);
      loadTransfers();
    } catch (error) {
      // silent
    }
  };

  const handleResume = async (sessionId) => {
    try {
      await api.resumeTransfer(sessionId);
      loadTransfers();
    } catch (error) {
      // silent
    }
  };

  const handleCancel = async (sessionId) => {
    try {
      await api.cancelTransfer(sessionId);
      loadTransfers();
    } catch (error) {
      // silent
    }
  };

  return (
    <div className="App">
      <header className="App-header">
        <div className="App-header-left">
          <h1>
            <Radio />
            RESILIENT
          </h1>
          <span className="App-header-tag">v1.0</span>
        </div>

        <ModeSelector mode={mode} onModeChange={setMode} />

        <div className="App-header-right">
          <div className="connection-status">
            <span className={`connection-dot ${connected ? '' : 'offline'}`} />
            {connected ? 'Connected' : 'Disconnected'}
          </div>
        </div>
      </header>

      <main className="App-main">
        {mode === 'sender' ? (
          <div className="sender-layout">
            <Dashboard stats={stats} currentMetrics={currentMetrics} />

            <AnimatedDataFlow currentMetrics={currentMetrics} />

            <div className="sender-top-row">
              <FileUpload onUpload={handleFileUpload} />
              <MetricsPanel
                metricsHistory={metricsHistory}
                currentMetrics={currentMetrics}
              />
            </div>

            <PacketLossSimulator
              currentMetrics={currentMetrics}
              uploadedFilePath={uploadedFilePath}
              uploadedFileName={uploadedFileName}
            />

            <ComparisonView
              currentMetrics={currentMetrics}
              uploadedFilePath={uploadedFilePath}
              uploadedFileName={uploadedFileName}
            />

            <TransferList
              transfers={transfers}
              onPause={handlePause}
              onResume={handleResume}
              onCancel={handleCancel}
            />
          </div>
        ) : (
          <>
            <ReceiverDashboard
              status={receiverStatus}
              receiverUrl={receiverUrl}
              onReceiverUrlChange={setReceiverUrl}
            />
            <ReceivedFilesList
              files={receivedFiles}
              receiverUrl={receiverUrl}
            />
          </>
        )}
      </main>

      <footer className="App-footer">
        RESILIENT v1.0.0 -- QUIC + Reed-Solomon Erasure Coding
      </footer>
    </div>
  );
}

export default App;
