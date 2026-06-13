export default function Dashboard({ config }) {
  if (!config) return null;

  const protocols = config.protocols || [];
  const tables = config.tables || [];
  const domains = config.mpls_domains || [];
  const prefixSids = config.sr_prefix_sids || [];

  return (
    <div className="dashboard-grid">
      {/* System Overview */}
      <div className="card">
        <h3 className="card-title">System</h3>
        <div className="stat-row">
          <span className="stat-label">Router ID</span>
          <span className="stat-value">{config.identity?.router_id || '—'}</span>
        </div>
        <div className="stat-row">
          <span className="stat-label">ASN</span>
          <span className="stat-value">{config.identity?.local_asn || '—'}</span>
        </div>
        <div className="stat-row">
          <span className="stat-label">Schema Version</span>
          <span className="stat-value">{config.schema_version}</span>
        </div>
        <div className="stat-row">
          <span className="stat-label">Hostname</span>
          <span className="stat-value">{config.hostname || '—'}</span>
        </div>
      </div>

      {/* Protocol Summary */}
      <div className="card card-wide">
        <h3 className="card-title">Protocols ({protocols.length})</h3>
        <table className="data-table">
          <thead>
            <tr><th>Name</th><th>Type</th><th>Table</th><th>Status</th></tr>
          </thead>
          <tbody>
            {protocols.map((p, i) => (
              <tr key={i}>
                <td>{p.name || '—'}</td>
                <td>{p.kind || '—'}</td>
                <td>{p.table || '—'}</td>
                <td><span className="status-up">● Active</span></td>
              </tr>
            ))}
            {protocols.length === 0 && (
              <tr><td colSpan="4" className="text-muted">No protocols configured</td></tr>
            )}
          </tbody>
        </table>
      </div>

      {/* Tables */}
      <div className="card">
        <h3 className="card-title">Routing Tables ({tables.length})</h3>
        <table className="data-table">
          <thead><tr><th>Name</th><th>Type</th><th>Kernel</th></tr></thead>
          <tbody>
            {tables.map((t, i) => (
              <tr key={i}>
                <td>{t.name}</td>
                <td>{t.nettype || 'ipv4'}</td>
                <td>{t.kernel_table || '—'}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {/* MPLS Information */}
      <div className="card">
        <h3 className="card-title">MPLS</h3>
        <div className="stat-row">
          <span className="stat-label">Domains</span>
          <span className="stat-value">{domains.length}</span>
        </div>
        {domains.map((d, i) => (
          <div key={i} className="stat-row">
            <span className="stat-label">{d.name}</span>
            <span className="stat-value">{d.label_ranges?.length || 0} ranges</span>
          </div>
        ))}
        {domains.length === 0 && <div className="text-muted">No MPLS domains configured</div>}
      </div>

      {/* Segment Routing */}
      <div className="card">
        <h3 className="card-title">Segment Routing</h3>
        <div className="stat-row">
          <span className="stat-label">Prefix SIDs</span>
          <span className="stat-value">{prefixSids.length}</span>
        </div>
        {prefixSids.slice(0, 5).map((s, i) => (
          <div key={i} className="stat-row stat-small">
            <span className="stat-label">{s.prefix}</span>
            <span className="stat-value">{s.domain}</span>
          </div>
        ))}
        {prefixSids.length === 0 && <div className="text-muted">No prefix SIDs configured</div>}
      </div>

      {/* Recent Activity — placeholder for RIB events */}
      <div className="card">
        <h3 className="card-title">RIB Activity</h3>
        <div className="text-muted" style={{padding: '20px 0', textAlign: 'center'}}>
          RIB event log will appear here<br/>
          <small>Subscribe to ProtocolSupervisor events</small>
        </div>
      </div>
    </div>
  );
}
