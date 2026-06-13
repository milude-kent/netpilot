import { useState } from 'react';

const API = 'http://127.0.0.1:8080';

export default function ConfigPanel({ config, onRefresh }) {
  const [mode, setMode] = useState('view'); // 'view' | 'edit'
  const [editing, setEditing] = useState(null);
  const [message, setMessage] = useState('');

  const handleEdit = () => {
    setEditing(JSON.stringify(config, null, 2));
    setMode('edit');
  };

  const handleSave = async () => {
    try {
      const parsed = JSON.parse(editing);
      const res = await fetch(`${API}/api/config/candidate`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(parsed),
      });
      if (res.ok) {
        setMessage('Candidate config updated. Commit to apply.');
        setMode('view');
        onRefresh();
      } else {
        setMessage(`Error: ${res.statusText}`);
      }
    } catch (e) {
      setMessage(`JSON Error: ${e.message}`);
    }
  };

  const handleCommit = async () => {
    const res = await fetch(`${API}/api/config/commit`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ author: 'web-ui', note: 'committed via Web UI' }),
    });
    if (res.ok) {
      setMessage('Committed successfully.');
      onRefresh();
    } else {
      setMessage(`Commit failed: ${res.statusText}`);
    }
  };

  return (
    <div className="config-panel">
      <div className="config-toolbar">
        <h3 className="card-title" style={{margin: 0}}>Running Configuration</h3>
        <div className="config-actions">
          {mode === 'view' ? (
            <button className="nav-btn" onClick={handleEdit}>Edit</button>
          ) : (
            <>
              <button className="nav-btn active" onClick={handleSave}>Save to Candidate</button>
              <button className="nav-btn" onClick={() => setMode('view')}>Cancel</button>
            </>
          )}
          <button className="nav-btn" onClick={handleCommit}>Commit</button>
        </div>
      </div>
      {message && <div className="message-bar">{message}</div>}
      <pre className="config-viewer">
        {mode === 'view'
          ? JSON.stringify(config, null, 2)
          : <textarea className="config-editor" value={editing}
              onChange={e => setEditing(e.target.value)} rows={40} />}
      </pre>
    </div>
  );
}
