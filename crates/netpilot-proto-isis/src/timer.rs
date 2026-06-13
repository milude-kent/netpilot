use tokio::time::{interval, Duration, Interval, MissedTickBehavior};

/// IS-IS protocol timers.
pub struct IsisTimers {
    /// Interval between hello transmissions on each interface.
    pub hello_interval: Interval,
    /// Interval between LSP refresh generations.
    pub lsp_refresh_interval: Interval,
    /// Interval between CSNP transmissions on broadcast interfaces.
    pub csnp_interval: Interval,
    /// Interval to check for expired LSPs.
    pub purge_interval: Interval,
    /// Interval to run SPF after topology changes.
    pub spf_interval: Interval,
}

impl IsisTimers {
    pub fn new(
        hello_secs: u64,
        lsp_refresh_secs: u64,
        csnp_secs: u64,
        purge_secs: u64,
        spf_debounce_secs: u64,
    ) -> Self {
        let mk_interval = |secs: u64| {
            let mut i = interval(Duration::from_secs(secs));
            i.set_missed_tick_behavior(MissedTickBehavior::Skip);
            i
        };
        Self {
            hello_interval: mk_interval(hello_secs),
            lsp_refresh_interval: mk_interval(lsp_refresh_secs),
            csnp_interval: mk_interval(csnp_secs),
            purge_interval: mk_interval(purge_secs),
            spf_interval: mk_interval(spf_debounce_secs),
        }
    }

    /// Default timers: hello every 10s, LSP refresh every 900s, CSNP every 10s,
    /// purge check every 60s, SPF debounce 2s.
    pub fn default_timers() -> Self {
        Self::new(10, 900, 10, 60, 2)
    }
}
