use crate::app::APP_CTX;

/// Apply-time scan over every upstream pool registry looking for a pool whose
/// `desc.id_string` matches `id_string`. If found, returns its `location_id`
/// so the freshly-compiled `ProxyPassLocationConfig` can adopt the existing
/// pool instead of allocating a fresh id (which would force a cold pool).
///
/// Cold path — runs once per location at config compile. O(total pools).
pub fn find_location_id_by_id_string(id_string: &str) -> Option<i64> {
    for pool in APP_CTX.h1_tcp_pools.list_pools() {
        if pool.desc.id_string == id_string {
            return Some(pool.desc.location_id);
        }
    }
    for pool in APP_CTX.h1_tls_pools.list_pools() {
        if pool.desc.id_string == id_string {
            return Some(pool.desc.location_id);
        }
    }
    for pool in APP_CTX.h1_uds_pools.list_pools() {
        if pool.desc.id_string == id_string {
            return Some(pool.desc.location_id);
        }
    }
    for pool in APP_CTX.h2_tcp_pools.list_pools() {
        if pool.desc.id_string == id_string {
            return Some(pool.desc.location_id);
        }
    }
    for pool in APP_CTX.h2_tls_pools.list_pools() {
        if pool.desc.id_string == id_string {
            return Some(pool.desc.location_id);
        }
    }
    for pool in APP_CTX.h2_uds_pools.list_pools() {
        if pool.desc.id_string == id_string {
            return Some(pool.desc.location_id);
        }
    }
    None
}
