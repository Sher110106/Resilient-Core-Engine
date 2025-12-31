import React, { useState, useEffect, useRef } from 'react';
import './App.css';
import FileUpload from './components/FileUpload';
import TransferList from './components/TransferList';
import Dashboard from './components/Dashboard';
import api from './services/api';

function App() {
  const [transfers, setTransfers] = useState([]);
  const [stats, setStats] = useState({ active: 0, completed: 0, failed: 0 });
  const wsRef = useRef(null);

  useEffect(() => {
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
  }, []);

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

  const handleFileUpload = async (file, priority) => {
    try {
      const result = await api.uploadAndTransfer(file, priority);
      console.log('Transfer started:', result);
      alert(`Transfer started! Session ID: ${result.session_id}`);
      // Reload transfers to show the new one
      setTimeout(loadTransfers, 500);
    } catch (error) {
      console.error('Error starting transfer:', error);
      alert(`Failed to start transfer: ${error.response?.data?.error || error.message}`);
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

  return (
    <div className="App">
      <header className="App-header">
        <h1>🚀 ChunkStream Pro</h1>
        <p>Smart File Transfer System with Erasure Coding</p>
      </header>

      <main className="App-main">
        <Dashboard stats={stats} />
        
        <FileUpload onUpload={handleFileUpload} />
        
        <TransferList 
          transfers={transfers}
          onPause={handlePause}
          onResume={handleResume}
          onCancel={handleCancel}
        />
      </main>

      <footer className="App-footer">
        <p>ChunkStream Pro v0.1.0 | Powered by Rust + React</p>
      </footer>
    </div>
  );
}

export default App;
