import { useState, useEffect, useCallback } from 'react';
import Dashboard from './components/Dashboard';
import ConfigPanel from './components/ConfigPanel';

const API = 'http://127.0.0.1:8080';

export default function App() {
  const [view, setView] = useState('dashboard');
  const [config, setConfig] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);

  const fetchConfig = useCallback(async () => {
    try {
      const res = await fetch(`${API}/api/config/running`);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();
      setConfig(data);
      setError(null);
    } catch (e) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { fetchConfig(); }, [fetchConfig]);

  // Auto-refresh every 10 seconds
  useEffect(() => {
    const interval = setInterval(fetchConfig, 10000);
    return () => clearInterval(interval);
  }, [fetchConfig]);

  return (
    <div className="app">
      <header className="header">
        <div className="header-left">
          <span className="logo">◈</span>
          <span className="title">NetPilot</span>
          <span className="subtitle">Routing Control</span>
        </div>
        <nav className="header-nav">
          <button className={`nav-btn ${view === 'dashboard' ? 'active' : ''}`}
            onClick={() => setView('dashboard')}>Dashboard</button>
          <button className={`nav-btn ${view === 'config' ? 'active' : ''}`}
            onClick={() => setView('config')}>Configuration</button>
        </nav>
        <div className="header-right">
          {error ? <span className="status-down">● API Offline</span> :
                    <span className="status-up">● Connected</span>}
        </div>
      </header>

      <main className="main">
        {loading && <div className="loading">Loading routing data...</div>}
        {!loading && view === 'dashboard' && <Dashboard config={config} />}
        {!loading && view === 'config' && <ConfigPanel config={config} onRefresh={fetchConfig} />}
      </main>
    </div>
  );
}
