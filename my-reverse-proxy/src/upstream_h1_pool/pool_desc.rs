/// Identity + presentation metadata for an h1 upstream pool. The pool is
/// keyed in the registry by `location_id`; `name` is the human-readable
/// label used in Prometheus metrics and admin contracts; `id_string` is
/// the apply-time logical identity (`"{listen_host}|{path}->{scheme}://
/// {host}:{port}"`) used to reuse an existing `location_id` across config
/// reloads — see `find_location_id_by_id_string`.
#[derive(Clone, Debug)]
pub struct PoolDesc {
    pub location_id: i64,
    pub name: String,
    pub id_string: String,
}
