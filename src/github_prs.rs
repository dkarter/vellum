//! GitHub-specific data shaping, isolated from the generic remote source protocol.
use std::{collections::HashMap, process::Command};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use crate::{
    remote::Page,
    source::{Cancellation, SourceItem, command_output_cancellable},
};

const PAGE_INFO: &str = "pageInfo { hasNextPage endCursor }";
const REVIEW_FIELDS: &str = "author { login } state submittedAt";
const REQUEST_FIELDS: &str = "requestedReviewer { ... on User { login } }";
const COMMENT_FIELDS: &str = "author { login }";
const THREAD_FIELDS: &str = "id isResolved comments(first:20) { nodes { author { login } } pageInfo { hasNextPage endCursor } }";
const CHECK_FIELDS: &str = "... on CheckRun { name status conclusion detailsUrl } ... on StatusContext { context state targetUrl }";

fn graphql(
    query: &str,
    variables: &[(&str, String)],
    cancellation: Option<&Cancellation>,
    cache: bool,
) -> Result<Value> {
    let mut command = Command::new("gh");
    command.args(["api", "graphql", "-f", &format!("query={query}")]);
    for (name, value) in variables {
        command.args(["-f", &format!("{name}={value}")]);
    }
    if cache {
        command.args(["--cache", "5m"]);
    }
    let output = command_output_cancellable(&mut command, "gh api graphql", cancellation)?;
    let value: Value = serde_json::from_str(&output).context("invalid GitHub GraphQL response")?;
    if value.get("errors").is_some() {
        bail!("GitHub GraphQL errors: {}", value["errors"]);
    }
    value
        .get("data")
        .cloned()
        .context("GitHub response has no data")
}

fn context(cancellation: Option<&Cancellation>) -> Result<(String, String)> {
    let query = "query($owner:String!,$name:String!) { viewer { login } repository(owner:$owner,name:$name) { nameWithOwner } }";
    let mut command = Command::new("gh");
    command.args([
        "api",
        "graphql",
        "--cache",
        "5m",
        "-F",
        "owner={owner}",
        "-F",
        "name={repo}",
        "-f",
        &format!("query={query}"),
    ]);
    let output =
        command_output_cancellable(&mut command, "GitHub repository identity", cancellation)?;
    let value: Value = serde_json::from_str(&output)?;
    if value.get("errors").is_some() {
        bail!("GitHub repository lookup failed: {}", value["errors"]);
    }
    Ok((
        value["data"]["repository"]["nameWithOwner"]
            .as_str()
            .context("repository not found")?
            .into(),
        value["data"]["viewer"]["login"]
            .as_str()
            .context("GitHub user not found")?
            .into(),
    ))
}

pub fn fetch(
    filter: &str,
    query: &str,
    cursor: Option<&str>,
    size: usize,
    cancellation: Option<&Cancellation>,
) -> Result<Page> {
    let (repo, user) = context(cancellation)?;
    fetch_page(
        &repo,
        &user,
        Request {
            filter,
            text: query,
            cursor,
            size,
            details: true,
        },
        cancellation,
    )
}

#[derive(Clone, Copy)]
struct Request<'a> {
    filter: &'a str,
    text: &'a str,
    cursor: Option<&'a str>,
    size: usize,
    details: bool,
}

fn fetch_page(
    repo: &str,
    user: &str,
    request: Request<'_>,
    cancellation: Option<&Cancellation>,
) -> Result<Page> {
    let Request {
        filter,
        text,
        cursor,
        size,
        details,
    } = request;
    let search = search_query(repo, user, filter, text)?;
    let display = if details {
        "number title url isDraft updatedAt body reviewDecision headRefName baseRefName"
    } else {
        ""
    };
    let checks = if details {
        format!(
            r#"commits(last:1) {{ nodes {{ commit {{ statusCheckRollup {{ id state contexts(first:100) {{ nodes {{ {CHECK_FIELDS} }} {PAGE_INFO} }} }} }} }} }}"#
        )
    } else {
        String::new()
    };
    let query = format!(
        r#"query($search:String!,$cursor:String) {{
      search(query:$search,type:ISSUE_ADVANCED,first:{size},after:$cursor) {{
        {PAGE_INFO}
        nodes {{ ... on PullRequest {{
          id state author {{ login }} {display}
          reviews(first:20) {{ nodes {{ {REVIEW_FIELDS} }} {PAGE_INFO} }}
          reviewRequests(first:20) {{ nodes {{ {REQUEST_FIELDS} }} {PAGE_INFO} }}
          reviewThreads(first:20) {{ nodes {{ {THREAD_FIELDS} }} {PAGE_INFO} }}
          {checks}
        }} }}
      }}
    }}"#
    );
    let mut variables = vec![("search", search)];
    if let Some(cursor) = cursor {
        variables.push(("cursor", cursor.into()));
    }
    let data = graphql(&query, &variables, cancellation, false)?;
    let connection = &data["search"];
    let mut items = Vec::new();
    let mut candidates = connection["nodes"]
        .as_array()
        .context("GitHub search has no nodes")?
        .clone();
    // Bound concurrency so busy PRs don't serialize dozens of subprocess calls,
    // without flooding GitHub or spawning a thread for every search result.
    for batch in candidates.chunks_mut(4) {
        std::thread::scope(|scope| -> Result<()> {
            let tasks: Vec<_> = batch
                .iter_mut()
                .map(|pr| {
                    scope.spawn(move || -> Result<()> {
                        if !details
                            && ((filter == "reviewed-closed"
                                && nodes(pr, "reviews").any(|review| authored_by(review, user)))
                                || (filter == "needs-review" && requested_from(pr, user)))
                        {
                            return Ok(());
                        }
                        complete_pr(pr, filter, user, cancellation)?;
                        if details && matches_filter(pr, user, filter) {
                            complete_checks(pr, cancellation)?;
                        }
                        Ok(())
                    })
                })
                .collect();
            for task in tasks {
                task.join()
                    .map_err(|_| anyhow::anyhow!("GitHub hydration worker panicked"))??;
            }
            Ok(())
        })?;
        for pr in batch.iter() {
            if matches_filter(pr, user, filter) {
                if !details {
                    items.push(SourceItem::new());
                    break;
                }
                items.push(item(pr, filter));
            }
        }
        if !details && !items.is_empty() {
            break;
        }
    }
    let next_cursor = next_cursor(connection)?;
    if cursor.is_some() && next_cursor.as_deref() == cursor {
        bail!("GitHub returned an unchanged cursor");
    }
    Ok(Page {
        items,
        next_cursor,
        ..Page::default()
    })
}

/// Cheap, exact positive probes avoid hydrating authored or already-reviewed
/// categories merely to light up their filter keys.
pub fn has_items(filter: &str, size: usize, cancellation: Option<&Cancellation>) -> Result<bool> {
    let (repo, user) = context(cancellation)?;
    let search = match filter {
        "needs-review" => {
            format!("repo:{repo} is:pr is:open -author:{user} user-review-requested:{user}")
        }
        "reviewed-closed" => format!("repo:{repo} is:pr is:closed reviewed-by:{user}"),
        _ => search_query(&repo, &user, filter, "")?,
    };
    let data = graphql(
        "query($search:String!) { search(query:$search,type:ISSUE_ADVANCED,first:1) { issueCount } }",
        &[("search", search)],
        cancellation,
        false,
    )?;
    if data["search"]["issueCount"]
        .as_u64()
        .context("GitHub search has no count")?
        > 0
    {
        return Ok(true);
    }
    if !matches!(filter, "needs-review" | "reviewed-closed") {
        return Ok(false);
    }
    crate::remote::any_page(|cursor| {
        fetch_page(
            &repo,
            &user,
            Request {
                filter,
                text: "",
                cursor,
                size,
                details: false,
            },
            cancellation,
        )
    })
}

fn search_query(repo: &str, user: &str, filter: &str, query: &str) -> Result<String> {
    let qualifier = match filter {
        "all-open" => String::from("is:open"),
        "mine-open" => format!("is:open author:{user}"),
        "needs-review" => format!(
            "is:open -author:{user} (review-involves:{user} OR reviewed-by:{user} OR commenter:{user})"
        ),
        "mine-closed" => format!("is:closed author:{user}"),
        "reviewed-closed" => format!("is:closed (reviewed-by:{user} OR commenter:{user})"),
        "" => String::from(""),
        _ => bail!("unknown GitHub PR filter: {filter}"),
    };
    // Treat input as title text, not qualifiers that could change the category
    // or escape the current repository. Local fuzzy matching remains immediate.
    let terms = query
        .split_whitespace()
        .map(|term| format!("\"{}\"", term.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join(" ");
    let title = if terms.is_empty() {
        String::new()
    } else {
        format!("in:title {terms}")
    };
    Ok(format!(
        "repo:{repo} is:pr {qualifier} {title} sort:updated-desc"
    ))
}

fn next_cursor(connection: &Value) -> Result<Option<String>> {
    if connection["pageInfo"]["hasNextPage"].as_bool() == Some(true) {
        let cursor = connection["pageInfo"]["endCursor"]
            .as_str()
            .filter(|cursor| !cursor.is_empty())
            .context("GitHub has another page but no cursor")?;
        Ok(Some(cursor.into()))
    } else {
        Ok(None)
    }
}

// Nested connections must also be paged: never silently miss a review or thread
// merely because a busy PR has more than one hundred of them.
fn complete_connection(
    id: &str,
    kind: &str,
    name: &str,
    fields: &str,
    connection: &mut Value,
    cancellation: Option<&Cancellation>,
) -> Result<()> {
    while let Some(cursor) = next_cursor(connection)? {
        let query = format!(
            "query($id:ID!,$cursor:String!) {{ node(id:$id) {{ ... on {kind} {{ {name}(first:100,after:$cursor) {{ nodes {{ {fields} }} {PAGE_INFO} }} }} }} }}"
        );
        let data = graphql(
            &query,
            &[("id", id.into()), ("cursor", cursor.clone())],
            cancellation,
            false,
        )?;
        let page = &data["node"][name];
        if next_cursor(page)?.as_deref() == Some(&cursor) {
            bail!("GitHub nested connection did not advance");
        }
        let nodes = page["nodes"]
            .as_array()
            .context("missing GitHub connection nodes")?;
        connection["nodes"]
            .as_array_mut()
            .context("missing GitHub initial nodes")?
            .extend(nodes.iter().cloned());
        connection["pageInfo"] = page["pageInfo"].clone();
    }
    Ok(())
}

fn complete_pr(
    pr: &mut Value,
    filter: &str,
    user: &str,
    cancellation: Option<&Cancellation>,
) -> Result<()> {
    let id = pr["id"].as_str().context("PR has no id")?.to_owned();
    for (name, fields) in [("reviews", REVIEW_FIELDS), ("reviewThreads", THREAD_FIELDS)] {
        complete_connection(
            &id,
            "PullRequest",
            name,
            fields,
            &mut pr[name],
            cancellation,
        )?;
    }
    if filter == "needs-review" {
        complete_connection(
            &id,
            "PullRequest",
            "reviewRequests",
            REQUEST_FIELDS,
            &mut pr["reviewRequests"],
            cancellation,
        )?;
    }
    let needs_comments = filter == "needs-review"
        || (filter == "reviewed-closed"
            && !nodes(pr, "reviews").any(|review| authored_by(review, user)));
    if !needs_comments {
        return Ok(());
    }
    for thread in pr["reviewThreads"]["nodes"]
        .as_array_mut()
        .context("missing review threads")?
    {
        if filter == "reviewed-closed" || thread["isResolved"] == false {
            let id = thread["id"]
                .as_str()
                .context("thread has no id")?
                .to_owned();
            complete_connection(
                &id,
                "PullRequestReviewThread",
                "comments",
                COMMENT_FIELDS,
                &mut thread["comments"],
                cancellation,
            )?;
        }
    }
    Ok(())
}

fn complete_checks(pr: &mut Value, cancellation: Option<&Cancellation>) -> Result<()> {
    let Some(rollup) = pr
        .get_mut("commits")
        .and_then(|commits| commits.get_mut("nodes"))
        .and_then(Value::as_array_mut)
        .and_then(|nodes| nodes.first_mut())
        .and_then(|node| node.get_mut("commit"))
        .and_then(|commit| commit.get_mut("statusCheckRollup"))
    else {
        return Ok(());
    };
    if let Some(id) = rollup["id"].as_str().map(str::to_owned) {
        complete_connection(
            &id,
            "StatusCheckRollup",
            "contexts",
            CHECK_FIELDS,
            &mut rollup["contexts"],
            cancellation,
        )?;
    }
    Ok(())
}

fn nodes<'a>(value: &'a Value, name: &str) -> impl Iterator<Item = &'a Value> {
    value[name]["nodes"].as_array().into_iter().flatten()
}

fn authored_by(value: &Value, user: &str) -> bool {
    value["author"]["login"]
        .as_str()
        .is_some_and(|login| login.eq_ignore_ascii_case(user))
}

fn latest_reviews(pr: &Value) -> HashMap<String, &Value> {
    let mut reviews = HashMap::new();
    for review in nodes(pr, "reviews") {
        if !matches!(
            review["state"].as_str(),
            Some("APPROVED" | "CHANGES_REQUESTED" | "DISMISSED")
        ) {
            continue;
        }
        if let Some(login) = review["author"]["login"].as_str() {
            reviews.insert(login.to_ascii_lowercase(), review);
        }
    }
    reviews
}

fn unresolved_count(pr: &Value, user: Option<&str>) -> usize {
    nodes(pr, "reviewThreads")
        .filter(|thread| {
            thread["isResolved"] == false
                && user.is_none_or(|user| {
                    nodes(thread, "comments").any(|comment| authored_by(comment, user))
                })
        })
        .count()
}

fn requested_from(pr: &Value, user: &str) -> bool {
    nodes(pr, "reviewRequests").any(|request| {
        request["requestedReviewer"]["login"]
            .as_str()
            .is_some_and(|login| login.eq_ignore_ascii_case(user))
    })
}

fn matches_filter(pr: &Value, user: &str, filter: &str) -> bool {
    let open = pr["state"] == "OPEN";
    match filter {
        "all-open" => open,
        "mine-open" => open && authored_by(pr, user),
        "mine-closed" => !open && authored_by(pr, user),
        "needs-review" => {
            open && !authored_by(pr, user)
                && (requested_from(pr, user)
                    || latest_reviews(pr)
                        .get(&user.to_ascii_lowercase())
                        .is_some_and(|review| review["state"] == "DISMISSED")
                    || unresolved_count(pr, Some(user)) > 0)
        }
        "reviewed-closed" => {
            !open
                && (nodes(pr, "reviews").any(|review| authored_by(review, user))
                    || nodes(pr, "reviewThreads").any(|thread| {
                        nodes(thread, "comments").any(|comment| authored_by(comment, user))
                    }))
        }
        _ => true,
    }
}

fn text<'a>(value: &'a Value, field: &str) -> &'a str {
    value[field].as_str().unwrap_or_default()
}

fn item(pr: &Value, filter: &str) -> SourceItem {
    let reviews = latest_reviews(pr);
    let approvals = reviews
        .values()
        .filter(|review| review["state"] == "APPROVED")
        .count();
    let rollup = &pr["commits"]["nodes"][0]["commit"]["statusCheckRollup"];
    let checks: Vec<_> = nodes(rollup, "contexts").collect();
    let mut passed = 0;
    let mut failed = 0;
    let mut pending = 0;
    let mut check_details = Vec::new();
    for check in &checks {
        let state = if check.get("context").is_some() {
            text(check, "state")
        } else if text(check, "status") != "COMPLETED" {
            "PENDING"
        } else {
            text(check, "conclusion")
        };
        match state {
            "SUCCESS" | "NEUTRAL" | "SKIPPED" => passed += 1,
            "FAILURE" | "ERROR" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED"
            | "STARTUP_FAILURE" | "STALE" => failed += 1,
            _ => pending += 1,
        }
        let name = check["name"]
            .as_str()
            .or_else(|| check["context"].as_str())
            .unwrap_or("check");
        let url = check["detailsUrl"]
            .as_str()
            .or_else(|| check["targetUrl"].as_str())
            .unwrap_or_default();
        check_details.push(format!("- **{name}: {state}**\n\n  {url}"));
    }
    let checks_summary = if checks.is_empty() {
        "no checks".into()
    } else {
        format!("✓ {passed}  ✗ {failed}  ◷ {pending}")
    };
    let approvals_summary = format!("✓ {approvals} approvals");
    let mut review_details: Vec<_> = reviews
        .iter()
        .map(|(login, review)| format!("- @{login}: {}", text(review, "state")))
        .collect();
    review_details.sort();
    let details = format!(
        "# PR #{} {}\n\n**{}{} · @{}**\n\n{} → {}\n\n{}\n\n## Reviews\n\n{}\n\n{}\n\nUnresolved review threads: {}\n\n## Checks\n\n{}\n\n{}\n\n---\n\n## Description\n\n{}",
        pr["number"],
        text(pr, "title"),
        text(pr, "state"),
        if pr["isDraft"] == true {
            " (draft)"
        } else {
            ""
        },
        text(&pr["author"], "login"),
        text(pr, "headRefName"),
        text(pr, "baseRefName"),
        text(pr, "url"),
        text(pr, "reviewDecision"),
        review_details.join("\n"),
        unresolved_count(pr, None),
        checks_summary,
        check_details.join("\n"),
        text(pr, "body")
    );
    serde_json::from_value(json!({
        "number": pr["number"], "title": pr["title"], "url": pr["url"], "author": pr["author"]["login"],
        "state": pr["state"], "draft": if pr["isDraft"] == true { "draft" } else { "" },
        "status": if pr["isDraft"] == true && pr["state"] == "OPEN" { json!("DRAFT") } else { pr["state"].clone() },
        "updated_at": pr["updatedAt"], "category": filter, "approvals": approvals_summary,
        "checks": checks_summary, "details": details,
    })).expect("object")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pr() -> Value {
        json!({"number": 42, "title":"Improve loading", "author":{"login":"someone"}, "state":"OPEN",
            "reviews":{"nodes":[]}, "reviewRequests":{"nodes":[]}, "reviewThreads":{"nodes":[]},
            "commits":{"nodes":[{"commit":{"statusCheckRollup":{"contexts":{"nodes":[]}}}}]}})
    }

    #[test]
    fn pal_020_all_open_includes_all_authors_and_drafts_but_excludes_closed() {
        let mut value = pr();
        for author in ["me", "someone"] {
            value["author"]["login"] = json!(author);
            for draft in [false, true] {
                value["isDraft"] = json!(draft);
                assert!(matches_filter(&value, "me", "all-open"));
            }
        }
        for state in ["CLOSED", "MERGED"] {
            value["state"] = json!(state);
            assert!(!matches_filter(&value, "me", "all-open"));
        }
        assert_eq!(
            search_query("owner/repo", "me", "all-open", "cache").unwrap(),
            "repo:owner/repo is:pr is:open in:title \"cache\" sort:updated-desc"
        );
        let config =
            crate::config::Config::parse(include_str!("../palettes/github-prs.toml")).unwrap();
        assert!(
            config
                .filters
                .choices
                .iter()
                .any(|choice| choice.value == "all-open" && choice.key.label() == "o")
        );
        assert_eq!(config.filters.initial.as_deref(), Some("mine-open"));
    }

    #[test]
    fn pal_020_review_checklist_classifies_requests_dismissals_and_threads() {
        let mut value = pr();
        assert!(!matches_filter(&value, "me", "needs-review"));
        value["reviewRequests"]["nodes"] = json!([{"requestedReviewer":{"login":"ME"}}]);
        assert!(matches_filter(&value, "me", "needs-review"));
        value["reviewRequests"]["nodes"] = json!([]);
        value["reviews"]["nodes"] = json!([{"author":{"login":"me"},"state":"DISMISSED"}]);
        assert!(matches_filter(&value, "me", "needs-review"));
        value["reviews"]["nodes"]
            .as_array_mut()
            .unwrap()
            .push(json!({"author":{"login":"me"},"state":"APPROVED"}));
        assert!(!matches_filter(&value, "me", "needs-review"));
        value["reviewThreads"]["nodes"] =
            json!([{"isResolved":false,"comments":{"nodes":[{"author":{"login":"me"}}]}}]);
        assert!(matches_filter(&value, "me", "needs-review"));
        value["reviewThreads"]["nodes"][0]["isResolved"] = json!(true);
        assert!(!matches_filter(&value, "me", "needs-review"));
        value["state"] = json!("MERGED");
        assert!(matches_filter(&value, "me", "reviewed-closed"));
        value["author"]["login"] = json!("me");
        assert!(matches_filter(&value, "me", "mine-closed"));
        value["state"] = json!("OPEN");
        assert!(matches_filter(&value, "me", "mine-open"));
        assert!(!matches_filter(&value, "me", "needs-review"));
    }

    #[test]
    fn pal_020_latest_reviews_and_checks_are_summarized() {
        let mut value = pr();
        value["reviews"]["nodes"] = json!([
            {"author":{"login":"a"},"state":"APPROVED"},
            {"author":{"login":"a"},"state":"DISMISSED"},
            {"author":{"login":"b"},"state":"APPROVED"}]);
        value["commits"]["nodes"][0]["commit"]["statusCheckRollup"]["contexts"]["nodes"] = json!([
            {"name":"test","status":"COMPLETED","conclusion":"SUCCESS"},
            {"name":"lint","status":"COMPLETED","conclusion":"FAILURE"},
            {"context":"deploy","state":"PENDING"}]);
        let item = item(&value, "mine-open");
        assert_eq!(item["approvals"], "✓ 1 approvals");
        assert_eq!(item["checks"], "✓ 1  ✗ 1  ◷ 1");
        assert!(item["details"].as_str().unwrap().contains("lint: FAILURE"));
        assert!(
            search_query("owner/repo", "me", "mine-closed", "")
                .unwrap()
                .contains("is:closed author:me")
        );
    }

    #[test]
    fn pal_020_server_search_keeps_repository_and_category_scope() {
        let search = search_query(
            "owner/repo",
            "me",
            "mine-open",
            "cache repo:other \"quoted\"",
        )
        .unwrap();
        assert!(search.starts_with("repo:owner/repo is:pr is:open author:me "));
        assert!(search.contains("in:title \"cache\" \"repo:other\" \"\\\"quoted\\\"\""));
    }
}
