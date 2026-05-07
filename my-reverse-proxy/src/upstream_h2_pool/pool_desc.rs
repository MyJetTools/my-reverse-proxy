/// Identity + presentation metadata for an h2 upstream pool. The pool is
/// keyed in the registry by `location_id`; `name` is used in Prometheus
/// metrics and admin contracts; `authority` (`host:port` for tcp/tls,
/// `localhost` for uds) is consumed by the liveness ping URI; `id_string`
/// is the apply-time logical identity used to reuse an existing
/// `location_id` across config reloads — see `find_location_id_by_id_string`.
#[derive(Clone, Debug)]
pub struct PoolDesc {
    pub location_id: i64,
    pub name: String,
    pub authority: String,
    pub id_string: String,
}
