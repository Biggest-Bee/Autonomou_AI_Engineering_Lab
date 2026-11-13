import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import Terminal from './Terminal';
import IDE from './IDE';
import './Dashboard.css';

interface DashboardProps {
  mode: 'public' | 'training';
  onTabChange?: (mode: 'public' | 'training') => void;
}

const Dashboard: React.FC<DashboardProps> = ({ mode, onTabChange }) => {
  const [systemInfo, setSystemInfo] = useState<string>('');
  const [activeTab, setActiveTab] = useState<'ask' | 'code'>('ask');
  const [isTrainingLocked, setIsTrainingLocked] = useState(true);
  const [adminPassword, setAdminPassword] = useState('');

  useEffect(() => {
    loadSystemInfo();
  }, []);

  const loadSystemInfo = async () => {
    try {
      const info = await invoke<string>('get_system_info');
      setSystemInfo(info);
    } catch (error) {
      console.error('Failed to load system info:', error);
    }
  };

  const handleTrainingAccess = () => {
    // Simple password check - in production, this would be more secure
    if (adminPassword === 'admin123') {
      setIsTrainingLocked(false);
    } else {
      alert('Invalid password');
    }
  };

  if (mode === 'training' && isTrainingLocked) {
    return (
      <div className="training-gate">
        <h2>Training & Admin Access</h2>
        <p>Enter administrative password to access training controls</p>
        <input
          type="password"
          value={adminPassword}
          onChange={(e) => setAdminPassword(e.target.value)}
          placeholder="Enter password"
        />
        <button onClick={handleTrainingAccess}>Access Training</button>
      </div>
    );
  }

  return (
    <div className="dashboard">
      <header className="dashboard-header">
        <div className="app-icon">🎩</div>
        <h1>Autonomous AI Engineering Lab</h1>
        <div className="system-info">{systemInfo}</div>
      </header>

      <div className="main-content">
        <nav className="sidebar">
          <div className="account-section">
            <h3>Accounts</h3>
            <button>Sign In with Google</button>
            <button>Account Settings</button>
          </div>
          
          <div className="mode-switch">
            <button
              className={mode === 'public' ? 'active' : ''}
              onClick={() => onTabChange?.('public')}
            >
              Public Use
            </button>
            <button
              className={mode === 'training' ? 'active' : ''}
              onClick={() => onTabChange?.('training')}
            >
              Training & Admin
            </button>
          </div>

          {mode === 'public' && (
            <div className="mode-tabs">
              <button
                className={activeTab === 'ask' ? 'active' : ''}
                onClick={() => setActiveTab('ask')}
              >
                Ask Mode
              </button>
              <button
                className={activeTab === 'code' ? 'active' : ''}
                onClick={() => setActiveTab('code')}
              >
                Code Mode
              </button>
            </div>
          )}

          {mode === 'training' && (
            <div className="training-controls">
              <h3>Training Controls</h3>
              <button>Start Training Loop</button>
              <button>View Loss Curves</button>
              <button>PRM Trajectory Graphs</button>
              <button>Container Health Metrics</button>
            </div>
          )}
        </nav>

        <main className="workspace">
          {mode === 'public' ? (
            <>
              <div className="ide-section">
                <IDE />
              </div>
              <div className="chat-section">
                {activeTab === 'ask' ? (
                  <div className="ask-mode">
                    <h3>Ask Mode</h3>
                    <p>Documentation and queries</p>
                    <textarea placeholder="Ask a question..."></textarea>
                    <button>Submit</button>
                  </div>
                ) : (
                  <div className="code-mode">
                    <h3>Code Mode</h3>
                    <p>Autonomous code generation and multi-file refactoring</p>
                    <textarea placeholder="Describe the code task..."></textarea>
                    <button>Generate Code</button>
                  </div>
                )}
              </div>
            </>
          ) : (
            <div className="training-view">
              <h3>Training Dashboard</h3>
              <div className="training-metrics">
                <div className="metric-card">
                  <h4>Container Health</h4>
                  <div className="metric-value">Healthy</div>
                </div>
                <div className="metric-card">
                  <h4>Current Loss</h4>
                  <div className="metric-value">0.0234</div>
                </div>
                <div className="metric-card">
                  <h4>PRM Score</h4>
                  <div className="metric-value">0.856</div>
                </div>
              </div>
              <div className="read-only-notice">
                ⚠️ IDE is in READ-ONLY mode during active training
              </div>
            </div>
          )}
        </main>

        <aside className="right-panel">
          <div className="autonomy-score">
            <h3>Autonomy Score (Aₛ)</h3>
            <div className="score-value">0.85</div>
            <div className="score-breakdown">
              <p>Autonomous Actions: 156</p>
              <p>Self-Corrections: 12</p>
              <p>Human Interventions: 3</p>
            </div>
          </div>

          <div className="execution-graph">
            <h3>Execution Graph</h3>
            <div className="graph-placeholder">
              DAG visualization placeholder
            </div>
          </div>

          <div className="terminal-panel">
            <Terminal />
          </div>
        </aside>
      </div>
    </div>
  );
};

export default Dashboard;