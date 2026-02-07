import React from 'react';
import { Activity, CheckCircle, XCircle, Shield, Zap } from 'lucide-react';
import './Dashboard.css';

function Dashboard({ stats, currentMetrics }) {
  const lossRate = currentMetrics ? (currentMetrics.loss_rate * 100).toFixed(1) : '0.0';
  const recovery = currentMetrics ? currentMetrics.recovery_capability.toFixed(1) : '0.0';

  return (
    <div className="dashboard">
      <div className="dashboard-card">
        <div className="dashboard-card-icon">
          <Activity size={16} />
        </div>
        <div className="dashboard-card-content">
          <span className="dashboard-card-value">{stats.active}</span>
          <span className="dashboard-card-label">Active Transfers</span>
        </div>
      </div>

      <div className="dashboard-card">
        <div className="dashboard-card-icon success">
          <CheckCircle size={16} />
        </div>
        <div className="dashboard-card-content">
          <span className="dashboard-card-value">{stats.completed}</span>
          <span className="dashboard-card-label">Completed</span>
        </div>
      </div>

      <div className="dashboard-card">
        <div className="dashboard-card-icon danger">
          <XCircle size={16} />
        </div>
        <div className="dashboard-card-content">
          <span className="dashboard-card-value">{stats.failed}</span>
          <span className="dashboard-card-label">Failed</span>
        </div>
      </div>

      <div className="dashboard-card">
        <div className="dashboard-card-icon warning">
          <Shield size={16} />
        </div>
        <div className="dashboard-card-content">
          <span className="dashboard-card-value">{lossRate}%</span>
          <span className="dashboard-card-label">Packet Loss</span>
        </div>
      </div>

      <div className="dashboard-card">
        <div className="dashboard-card-icon info">
          <Zap size={16} />
        </div>
        <div className="dashboard-card-content">
          <span className="dashboard-card-value">{recovery}%</span>
          <span className="dashboard-card-label">Recovery</span>
        </div>
      </div>
    </div>
  );
}

export default Dashboard;
