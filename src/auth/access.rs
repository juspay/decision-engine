//! What each request requires of the session making it.
//!
//! The mapping lives here, in one table, rather than in each handler: a route added later is
//! covered the day it is written, instead of depending on whoever adds it to remember that
//! sessions can be limited.
//!
//! Adding a permission is a two-line change — a variant on [`Permission`], and the routes that
//! need it named below. The token, the middleware, and the dashboard contract stay as they are.

use super::Permission;
use axum::http::Method;

/// The POST endpoints that only read.
///
/// Decision Engine serves several reads over POST — they take a request body rather than a query
/// string — so the method on its own cannot separate reading from writing. Treating every non-GET
/// as a write would leave a read-only user unable to list rules or open one, which is the entire
/// thing they are meant to be able to do.
///
/// Entries are matched against the *routed path pattern*, never the request URL, so no path
/// parameter can be crafted to resemble one of these.
const READ_ONLY_POST_ROUTES: &[&str] = &[
    "/routing/list/:created_by",
    "/routing/list/active/:created_by",
    "/routing/evaluate",
    "/rule/get",
    "/merchant-account/:merchant-id/seed-costs/simulate",
];

/// The permission a request needs.
///
/// GET (with the other safe methods) is taken to be free of side effects — a rule HTTP already
/// imposes, and one that holds across every route here. Everything else counts as a write unless
/// it is named above, so a route added later requires a write until someone decides otherwise.
/// That is the safe direction for the mistake to fall: a limited session meets a visible 403,
/// rather than silently gaining a way to change something.
///
/// When a later permission covers an area of its own, this is where it is claimed — a branch on
/// the matched path returning, say, `AnalyticsRead` for `/analytics/...`.
pub fn required_permission(method: &Method, matched_path: Option<&str>) -> Permission {
    if matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
        return Permission::RoutingRead;
    }

    let is_read = *method == Method::POST
        && matched_path.is_some_and(|path| READ_ONLY_POST_ROUTES.contains(&path));

    if is_read {
        Permission::RoutingRead
    } else {
        Permission::RoutingWrite
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_served_over_post_stay_reachable() {
        // The failure this exists to prevent: a read-only user who cannot see the rules.
        for path in READ_ONLY_POST_ROUTES {
            assert_eq!(
                required_permission(&Method::POST, Some(path)),
                Permission::RoutingRead,
                "{path} must stay readable"
            );
        }
    }

    #[test]
    fn writes_are_refused_even_where_a_read_lives_nearby() {
        // `/rule/get` and `/rule/create` differ only in the last word.
        for path in [
            "/rule/create",
            "/rule/update",
            "/rule/delete",
            "/routing/create",
            "/routing/activate",
        ] {
            assert_eq!(
                required_permission(&Method::POST, Some(path)),
                Permission::RoutingWrite,
                "{path} must require a write"
            );
        }
    }

    #[test]
    fn a_shared_path_is_classified_by_method_not_by_name() {
        // `/merchant-account/:id/seed-costs` answers GET, PUT and DELETE. Allowing the read must
        // not carry the writes with it.
        let path = Some("/merchant-account/:merchant-id/seed-costs");
        assert_eq!(
            required_permission(&Method::GET, path),
            Permission::RoutingRead
        );
        assert_eq!(
            required_permission(&Method::PUT, path),
            Permission::RoutingWrite
        );
        assert_eq!(
            required_permission(&Method::DELETE, path),
            Permission::RoutingWrite
        );
    }

    #[test]
    fn an_unclassified_post_requires_a_write() {
        // A route added later needs a write until someone says otherwise.
        assert_eq!(
            required_permission(&Method::POST, Some("/some/new/endpoint")),
            Permission::RoutingWrite
        );
        assert_eq!(
            required_permission(&Method::POST, None),
            Permission::RoutingWrite
        );
    }

    #[test]
    fn a_concrete_url_does_not_pass_as_a_pattern() {
        // Only the routed pattern matches, so this can never be spoofed through the URL.
        assert_eq!(
            required_permission(
                &Method::POST,
                Some("/routing/list/pro_Cpbj6dzbVqQIqSdRpEai")
            ),
            Permission::RoutingWrite
        );
    }
}
