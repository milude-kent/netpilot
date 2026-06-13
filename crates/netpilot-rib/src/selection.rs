use crate::route::RouteEntry;

/// Select the best route from candidates using BIRD2 ordering:
/// 1. Higher preference wins
/// 2. Lower metric wins
/// 3. ECMP if all criteria equal
pub fn select_best(candidates: &[RouteEntry]) -> Option<&RouteEntry> {
    candidates.iter()
        .filter(|e| e.state == crate::route::RouteState::Active)
        .max_by(|a, b| {
            b.preference.cmp(&a.preference)
                .then_with(|| b.metric.unwrap_or(u32::MAX).cmp(&a.metric.unwrap_or(u32::MAX)))
        })
}

/// Find all ECMP paths (equal to best in preference and metric).
pub fn find_ecmp<'a>(candidates: &'a [RouteEntry], best: &RouteEntry) -> Vec<&'a RouteEntry> {
    candidates.iter()
        .filter(|e| e.state == crate::route::RouteState::Active)
        .filter(|e| e.preference == best.preference && e.metric == best.metric)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::RouteKey;

    #[test]
    fn selects_higher_preference() {
        let a = RouteEntry::new(RouteKey::prefix("10.0.0.0/8"), "master", "bgp", 100);
        let b = RouteEntry::new(RouteKey::prefix("10.0.0.0/8"), "master", "ospf", 150);
        let routes = vec![a, b];
        let best = select_best(&routes).unwrap();
        assert_eq!(best.source_protocol, "bgp");
        assert_eq!(best.preference, 100);
    }

    #[test]
    fn selects_lower_metric_when_same_preference() {
        let a = RouteEntry::new(RouteKey::prefix("10.0.0.0/8"), "master", "bgp", 100).with_metric(50);
        let b = RouteEntry::new(RouteKey::prefix("10.0.0.0/8"), "master", "isis", 100).with_metric(10);
        let routes = vec![a, b];
        let best = select_best(&routes).unwrap();
        assert_eq!(best.source_protocol, "isis");
        assert_eq!(best.metric, Some(10));
    }

    #[test]
    fn ecmp_finds_equal_paths() {
        let a = RouteEntry::new(RouteKey::prefix("10.0.0.0/8"), "master", "bgp", 100).with_metric(10);
        let b = RouteEntry::new(RouteKey::prefix("10.0.0.0/8"), "master", "isis", 100).with_metric(10);
        let routes = vec![a, b];
        let best = select_best(&routes).unwrap();
        let ecmp = find_ecmp(&routes, best);
        assert_eq!(ecmp.len(), 2);
    }
}
