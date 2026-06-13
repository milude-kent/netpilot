use netpilot_config::ChannelLimits;
use netpilot_rib::route::RouteEntry;

/// Statistics for a protocol channel.
#[derive(Clone, Debug, Default)]
pub struct ChannelStats {
    pub imported: u64,
    pub filtered_imports: u64,
    pub exported: u64,
    pub filtered_exports: u64,
}

/// A protocol channel binding one protocol to one table with import/export filters.
#[derive(Clone, Debug)]
pub struct ProtocolChannel {
    pub protocol_name: String,
    pub table: String,
    pub import_limit: Option<u32>,
    pub export_limit: Option<u32>,
    pub import_keep_filtered: bool,
    pub stats: ChannelStats,
}

impl ProtocolChannel {
    pub fn new(protocol_name: &str, table: &str) -> Self {
        Self {
            protocol_name: protocol_name.to_string(),
            table: table.to_string(),
            import_limit: None,
            export_limit: None,
            import_keep_filtered: false,
            stats: ChannelStats::default(),
        }
    }

    pub fn with_limits(mut self, limits: &ChannelLimits) -> Self {
        self.import_limit = limits.import_limit;
        self.export_limit = limits.export_limit;
        self.import_keep_filtered = limits.import_limit_action.is_some();
        self
    }

    /// Apply import processing. Returns Some(route) if accepted, None if filtered.
    /// In the full implementation, this runs the import filter VM.
    pub fn import_filter(&mut self, route: &RouteEntry) -> Option<RouteEntry> {
        // Check import limit
        if let Some(limit) = self.import_limit {
            if self.stats.imported >= limit as u64 {
                self.stats.filtered_imports += 1;
                return None;
            }
        }
        self.stats.imported += 1;
        Some(route.clone())
    }

    /// Apply export processing. Returns Some(route) if the route should be exported.
    pub fn export_filter(&mut self, route: &RouteEntry) -> Option<RouteEntry> {
        if let Some(limit) = self.export_limit {
            if self.stats.exported >= limit as u64 {
                self.stats.filtered_exports += 1;
                return None;
            }
        }
        self.stats.exported += 1;
        Some(route.clone())
    }
}
