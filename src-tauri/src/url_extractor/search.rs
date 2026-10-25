//! `DuckDuckGo` search URL encoding and redirect link unwrapping.

/// The `DuckDuckGo` results page URL under `base` for `query`.
pub(super) fn search_url(base: &str, query: &str) -> String {
    let encoded: String = form_urlencoded::byte_serialize(query.trim().as_bytes()).collect();
    format!("{base}?q={encoded}")
}

/// `DuckDuckGo` routes every result through `/l/?uddg=<target>&rut=<signature>`
/// and encodes the target as a query value, so the wrapper comes off here.
pub(super) fn unwrap_redirects(markdown: &str) -> String {
    let marker = "](//duckduckgo.com/l/?";
    let mut out = String::with_capacity(markdown.len());
    let mut rest = markdown;
    while let Some(at) = rest.find(marker) {
        let from = at + marker.len();
        let to = from + rest[from..].find(')').unwrap_or(rest.len() - from);
        let query = &rest[from..to];
        let target = form_urlencoded::parse(query.as_bytes())
            .find(|(key, _)| key == "uddg")
            .map_or_else(|| query.to_string(), |(_, value)| value.into_owned());
        out.push_str(&rest[..at]);
        out.push_str("](");
        out.push_str(&target);
        out.push(')');
        rest = rest[to..].strip_prefix(')').unwrap_or(&rest[to..]);
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::{search_url, unwrap_redirects};

    #[test]
    fn the_query_is_encoded_into_the_results_page_url() {
        assert_eq!(
            search_url("https://html.duckduckgo.com/html/", "rust borrow checker"),
            "https://html.duckduckgo.com/html/?q=rust+borrow+checker"
        );
    }

    #[test]
    fn characters_that_would_break_the_query_string_are_escaped() {
        assert_eq!(
            search_url("https://html.duckduckgo.com/html/", "C++ & rust"),
            "https://html.duckduckgo.com/html/?q=C%2B%2B+%26+rust"
        );
    }

    #[test]
    fn padding_around_the_query_is_not_part_of_it() {
        assert_eq!(
            search_url("https://html.duckduckgo.com/html/", "  mcp  "),
            "https://html.duckduckgo.com/html/?q=mcp"
        );
    }

    #[test]
    fn a_result_link_points_at_the_page_it_wraps() {
        let wrapped = "[LogRocket](//duckduckgo.com/l/?uddg=https%3A%2F%2Fblog.logrocket.com%2Fintroducing%2Drust%2Dborrow%2Dchecker%2F&rut=a5a24a)";

        assert_eq!(
            unwrap_redirects(wrapped),
            "[LogRocket](https://blog.logrocket.com/introducing-rust-borrow-checker/)"
        );
    }

    #[test]
    fn every_redirect_on_the_page_is_unwrapped() {
        let wrapped = "[a](//duckduckgo.com/l/?uddg=https%3A%2F%2Fa.dev&rut=1) text ![i](//duckduckgo.com/l/?uddg=https%3A%2F%2Fb.dev%2Ffavicon.ico&rut=2)";

        assert_eq!(
            unwrap_redirects(wrapped),
            "[a](https://a.dev) text ![i](https://b.dev/favicon.ico)"
        );
    }

    #[test]
    fn a_link_that_is_not_a_redirect_is_left_alone() {
        let markdown = "[docs](https://doc.rust-lang.org/book/) and [nav](/html/)";

        assert_eq!(unwrap_redirects(markdown), markdown);
    }
}
