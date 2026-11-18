//! Radios of the app (`features/mixes/api.ts` `fetchRadioBatch`, `radioPlayer.ts`,
//! `RadioIndicator.tsx`): the engine's `Radio` does the endless refill, this feeds it songs.
use dioxus::prelude::*;
use orphyn_jellyfin::endpoints::{Session, DEFAULT_FIELDS};
use orphyn_jellyfin::{BaseItemDto, ItemsParams};
use orphyn_player::music::radio::{first_queue, radio, Fetcher, RadioKind, RadioSeed};
use orphyn_player::music::MusicPlayer;

use crate::design::Icon;
use crate::i18n::use_i18n;
use crate::state::use_services;

/// Songs of one artist, random order (`songs()` of `mixes/api.ts`).
async fn artist_songs(s: &Session, artist: &str, limit: u32) -> Vec<BaseItemDto> {
    let mut fields: Vec<String> = DEFAULT_FIELDS.iter().map(|f| f.to_string()).collect();
    fields.push("Genres".into());
    let p = ItemsParams {
        include_item_types: vec!["Audio".into()],
        artist_ids: vec![artist.to_string()],
        sort_by: vec!["Random".into()],
        page_size: Some(limit),
        fields: Some(fields),
        ..ItemsParams::default()
    };
    s.items_page(&p, 0)
        .await
        .map(|p| p.items)
        .unwrap_or_default()
}

/// More songs for a radio seeded by a song, album or artist: its Instant Mix, the Instant Mix of
/// the song being played, and a few songs of similar artists, interleaved. Not deduplicated (the
/// radio does it).
pub async fn fetch_radio_batch(s: &Session, seed: &str, current: Option<&str>) -> Vec<BaseItemDto> {
    let current_mix = async {
        match current {
            Some(c) if c != seed => s.instant_mix(c, 30).await.unwrap_or_default(),
            _ => Vec::new(),
        }
    };
    let similar = async {
        let artists: Vec<String> = s
            .similar(seed, 6)
            .await
            .unwrap_or_default()
            .into_iter()
            .filter(|a| a.is_type("MusicArtist"))
            .take(4)
            .map(|a| a.id)
            .collect();
        // Four small requests, one after the other.
        let mut songs = Vec::new();
        for id in &artists {
            songs.extend(artist_songs(s, id, 4).await);
        }
        songs
    };
    let (seed_mix, current_mix, similar) = tokio::join!(
        async { s.instant_mix(seed, 50).await.unwrap_or_default() },
        current_mix,
        similar
    );
    interleave(&[seed_mix, current_mix, similar])
}

/// Round-robin over the lists so a refill is not "all of one source" in a row.
pub fn interleave(lists: &[Vec<BaseItemDto>]) -> Vec<BaseItemDto> {
    let mut out = Vec::new();
    let longest = lists.iter().map(Vec::len).max().unwrap_or(0);
    for i in 0..longest {
        for l in lists {
            if let Some(t) = l.get(i) {
                out.push(t.clone());
            }
        }
    }
    out
}

/// Starts a radio from a song, an album or an artist (its item id). `None` when it is empty or the
/// server cannot be reached (`startRadio`).
pub async fn start_radio(player: MusicPlayer, session: Session, id: String) -> Option<String> {
    let item = session.item(&id).await.ok()?;
    let kind = if item.is_type("MusicArtist") {
        RadioKind::Artist
    } else if item.is_type("MusicAlbum") {
        RadioKind::Album
    } else {
        RadioKind::Song
    };
    let batch = fetch_radio_batch(&session, &id, None).await;
    let first: Vec<BaseItemDto> = if kind == RadioKind::Song {
        vec![item.clone()]
    } else {
        Vec::new()
    };
    let queue = first_queue(&first, &batch);
    if queue.is_empty() {
        return None;
    }
    player.play(&queue, Some(0), Some(false));
    let name = item.name().to_string();
    let fetcher: Fetcher = std::sync::Arc::new(move |seed, current| {
        let s = session.clone();
        Box::pin(async move { fetch_radio_batch(&s, &seed, current.as_deref()).await })
    });
    radio(&player).start(
        RadioSeed {
            id,
            name: name.clone(),
            kind,
        },
        &queue,
        fetcher,
    );
    Some(name)
}

/// The running radio's seed as a signal (`useRadio`).
pub fn use_radio_seed() -> Signal<Option<RadioSeed>> {
    let player = use_services().music;
    let r = radio(&player);
    let mut seed = use_signal(|| r.seed());
    use_hook(move || {
        let mut rx = r.subscribe();
        spawn(async move {
            while rx.changed().await.is_ok() {
                let s = rx.borrow_and_update().clone();
                seed.set(s);
            }
        });
    });
    seed
}

/// "Radio · <seed>" pill with a stop button, shown by the queue while a radio is running.
#[component]
pub fn RadioIndicator() -> Element {
    let t = use_i18n();
    let player = use_services().music;
    let seed = use_radio_seed();
    let Some(seed) = seed() else {
        return rsx! {};
    };
    rsx! {
        span { class: "jrx-radio", role: "status",
            Icon { name: "radio", size: 12 }
            {t.tf("mixes:radio.indicator", &[("name", seed.name.clone())])}
            button {
                r#type: "button",
                class: "jrm-heart",
                "aria-label": t.t("mixes:radio.stop"),
                onmousedown: move |e| e.stop_propagation(),
                onclick: move |_| radio(&player).stop(),
                Icon { name: "x", size: 12 }
            }
        }
    }
}
