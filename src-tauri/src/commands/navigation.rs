/// Resolve a logical tray/IPC feed target to its TikTok URL.
/// Kept exhaustive — unknown targets fall back to the For You feed.
/// `pub(crate)`: the tray menu dispatches through this same list, so there is
/// exactly ONE source of truth for destination URLs (v2.1.0 audit round 2).
///
/// v2.1.0 round 5: the `navigate_to` command was removed — it had no callers
/// (the tray dispatches through `feed_url` directly), so it was dead IPC
/// surface with an unnecessary capability grant.
/// Resolve a logical tray feed target to its TikTok URL.
/// Kept exhaustive — unknown targets fall back to the For You feed.
pub(crate) fn feed_url(target: &str) -> &'static str {
    match target {
        // v2.1.0 round 5 (F27): the tray menu item id is "for_you" while this
        // match historically accepted "foryou" — the fallback silently sent
        // the For You entry to the bare root URL. Both spellings are accepted
        // now so the tray id and the IPC target can never diverge again.
        "for_you" | "foryou" => "https://www.tiktok.com/foryou",
        "following" => "https://www.tiktok.com/following",
        "explore" => "https://www.tiktok.com/explore",
        "live" => "https://www.tiktok.com/live",
        "friends" => "https://www.tiktok.com/friends",
        "upload" => "https://www.tiktok.com/upload",
        // v2.1.0: TikTok's Direct Messages page — surfaced in the tray menu and
        // reachable through the same audited whitelist path as the feeds.
        "messages" => "https://www.tiktok.com/messages",
        _ => "https://www.tiktok.com/",
    }
}
