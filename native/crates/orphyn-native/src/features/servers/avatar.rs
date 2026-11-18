//! `UserAvatar` (`Avatar.tsx`): the Jellyfin profile picture, or the initials.
use dioxus::prelude::*;
use orphyn_core::accounts::ServerAccount;

fn initials(name: &str) -> String {
    let mut parts = name.split_whitespace();
    let a = parts.next().and_then(|p| p.chars().next());
    let b = parts.next().and_then(|p| p.chars().next());
    match (a, b) {
        (Some(a), Some(b)) => format!("{a}{b}").to_uppercase(),
        (Some(a), None) => a.to_uppercase().to_string(),
        _ => String::new(),
    }
}

#[component]
pub fn UserAvatar(account: ServerAccount, #[props(default = 28)] size: u32) -> Element {
    let src = format!(
        "{}/Users/{}/Images/Primary?maxWidth={}",
        account.url,
        account.user_id,
        size * 2
    );
    let mut failed = use_signal(|| false);
    rsx! {
        span { class: "jr-avatar", style: "width: {size}px; height: {size}px;", "aria-hidden": "true",
            if failed() {
                "{initials(&account.user_name)}"
            } else {
                img { src: "{src}", alt: "", onerror: move |_| failed.set(true) }
            }
        }
    }
}
