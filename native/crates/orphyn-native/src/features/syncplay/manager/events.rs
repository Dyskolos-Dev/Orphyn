//! Server events and commands of the manager (second half of `manager.ts`): membership changes,
//! the group's queue, and the scheduled Unpause / Pause / Seek / Stop.
use super::*;

impl SyncPlayManager {
    pub(super) fn on_user(&self, user: String, joined: bool) {
        let Some(mut group) = self.state().group.clone() else { return };
        if joined {
            group.participants.push(user.clone());
        } else if let Some(i) = group.participants.iter().position(|p| *p == user) {
            group.participants.remove(i);
        }
        self.update(|s| s.group = Some(group));
        self.refresh_members();
        self.notify(if joined { Notice::UserJoined(user) } else { Notice::UserLeft(user) });
    }

    pub(super) fn on_group_joined(&self, group: SyncPlayGroup) {
        let (was_rejoin, share) = {
            let mut g = self.lock();
            let r = std::mem::take(&mut g.rejoining);
            (r, g.share_on_join)
        };
        let name = group.name.clone();
        self.update(|s| {
            s.phase = Phase::Joined;
            s.group_state = group.state;
            s.group = Some(group);
            s.sheet_open = false;
        });
        self.refresh_members();
        self.notify(if was_rejoin { Notice::Reconnected } else { Notice::Joined(name) });
        if share {
            self.lock().share_on_join = false;
            self.share_local(false);
        }
    }

    pub(super) fn on_error(&self, kind: SyncPlayErrorKind) {
        let phase = self.state().phase;
        match kind {
            SyncPlayErrorKind::GroupDoesNotExist => {
                let (rejoining, alone) = {
                    let g = self.lock();
                    (g.rejoining, g.others_seen == 0)
                };
                if rejoining && alone {
                    // We were alone: the group went away with our connection. Open it again.
                    let name = self
                        .state()
                        .group
                        .as_ref()
                        .map(|g| g.name.clone())
                        .unwrap_or_default();
                    self.update(|s| {
                        s.queue = None;
                        s.current_item = None;
                        s.current_kind = None;
                    });
                    {
                        let mut g = self.lock();
                        g.queue_key.clear();
                        g.share_on_join = true;
                    }
                    self.send(move |c| async move { sp::create_group(&c, &name).await });
                    return;
                }
                self.teardown();
                self.notify(Notice::GroupClosed);
            }
            SyncPlayErrorKind::NotInGroup => {
                let (leaving, rejoining) = {
                    let g = self.lock();
                    (g.leaving, g.rejoining)
                };
                if phase == Phase::Joined && !leaving && !rejoining {
                    self.rejoin();
                }
            }
            SyncPlayErrorKind::CreateGroupDenied | SyncPlayErrorKind::JoinGroupDenied => {
                if phase == Phase::Connecting || self.lock().rejoining {
                    self.teardown();
                }
                self.notify(if kind == SyncPlayErrorKind::CreateGroupDenied {
                    Notice::CreateDenied
                } else {
                    Notice::JoinDenied
                });
            }
            SyncPlayErrorKind::LibraryAccessDenied => {
                if phase == Phase::Connecting {
                    self.teardown();
                }
                self.notify(Notice::LibraryDenied);
            }
        }
    }

    pub(super) fn refresh_members(&self) {
        let s = self.state();
        let Some(group) = &s.group else { return };
        let members =
            sp::describe_members(&group.participants, &s.self_name, s.self_ready, s.group_state);
        self.lock().others_seen = members.len().saturating_sub(1);
        self.update(|s| s.members = members);
    }

    fn author(&self) -> Option<String> {
        let s = self.state();
        let participants: &[String] = s.group.as_ref().map_or(&[], |g| &g.participants);
        sp::likely_author(participants, &s.self_name)
    }

    pub(super) fn on_state_update(&self, state: GroupState, reason: &str) {
        self.update(|s| s.group_state = state);
        self.refresh_members();
        if state == GroupState::Paused && reason == "Pause" && !self.is_mine(LocalKind::Pause) {
            self.notify(Notice::Paused(self.author()));
        } else if state == GroupState::Playing
            && reason == "Unpause"
            && !self.is_mine(LocalKind::Unpause)
        {
            self.notify(Notice::Resumed(self.author()));
        } else if reason == "Seek" && !self.is_mine(LocalKind::Seek) {
            self.notify(Notice::Seeked(self.author()));
        } else if state == GroupState::Waiting && reason == "Buffer" && self.state().self_ready {
            let now = now_ms();
            let due = now - self.lock().last_waiting_notice > WAITING_NOTICE_GAP_MS;
            if due {
                self.lock().last_waiting_notice = now;
                self.notify(Notice::Waiting(sp::waiting_for(&self.state().members)));
            }
        }
    }

    async fn resolve_items(&self, ids: &[String]) -> Result<HashMap<String, BaseItemDto>, String> {
        let (missing, conn) = {
            let g = self.lock();
            let mut missing: Vec<String> = Vec::new();
            for id in ids {
                if !g.items.contains_key(id) && !missing.contains(id) {
                    missing.push(id.clone());
                }
            }
            (missing, g.conn.as_ref().map(|c| (c.client.clone(), c.user_id.clone())))
        };
        if let (false, Some((client, user))) = (missing.is_empty(), conn) {
            let found = sp::fetch_items_by_ids(&client, &user, &missing)
                .await
                .map_err(|e| e.to_string())?;
            self.lock().items.extend(found);
        }
        Ok(self.lock().items.clone())
    }

    pub(super) async fn apply_queue(&self, queue: PlayQueue) {
        let entry = playing_entry(&Some(queue.clone()));
        let before = playing_entry(&self.state().queue);
        let key = format!(
            "{}|{}|{}",
            queue.last_update,
            queue.playing_item_index,
            queue
                .playlist
                .iter()
                .map(|e| e.playlist_item_id.as_str())
                .collect::<Vec<_>>()
                .join(",")
        );
        self.update(|s| s.queue = Some(queue.clone()));
        let known = {
            let mut g = self.lock();
            if key == g.queue_key {
                return;
            }
            let known = !g.queue_key.is_empty();
            g.queue_key = key.clone();
            known
        };
        let resets = QUEUE_RESETS.contains(&queue.reason.as_str());
        let ids: Vec<String> = queue.playlist.iter().map(|e| e.item_id.clone()).collect();
        let same_entry = entry.is_some()
            && before.as_ref().map(|b| &b.playlist_item_id) == entry.as_ref().map(|e| &e.playlist_item_id);
        if known && same_entry && !resets {
            // Entries added, removed or moved around the one playing: playback goes on untouched.
            let music = self.lock().music.clone();
            if self.state().current_kind == Some(MediaKind::Audio) {
                if let (Some(music), Ok(items)) = (music, self.resolve_items(&ids).await) {
                    if key == self.lock().queue_key {
                        music.extend(playable_of(&queue, &items).0);
                    }
                }
            }
            return;
        }
        self.clear_command_task();
        {
            let mut g = self.lock();
            g.allow_play = false;
            g.last_unpause = None;
        }
        self.set_rate(1.0);
        let Some(entry) = entry else {
            self.update(|s| {
                s.current_item = None;
                s.current_kind = None;
            });
            return;
        };
        let Ok(items) = self.resolve_items(&ids).await else {
            self.notify(Notice::Error);
            return;
        };
        if key != self.lock().queue_key {
            return; // a newer queue arrived meanwhile
        }
        let Some(item) = items.get(&entry.item_id).cloned() else {
            // Not in this user's libraries: say so, and let the group go on without waiting.
            self.update(|s| {
                s.current_item = None;
                s.current_kind = None;
            });
            self.notify(Notice::ItemUnavailable);
            self.set_ignore_wait(true);
            return;
        };
        self.set_ignore_wait(false);
        let kind = if item.media_type.as_deref() == Some("Video") {
            MediaKind::Video
        } else {
            MediaKind::Audio
        };
        self.update(|s| {
            s.current_item = Some(item.clone());
            s.current_kind = Some(kind);
        });
        if !self.is_mine(LocalKind::Queue) && resets {
            self.notify(Notice::NowPlaying { title: item_title(&item), by: self.author() });
        }
        let start = queue.start_position_ticks / TICKS_PER_SECOND;
        let (video, music) = {
            let g = self.lock();
            (g.video.clone(), g.music.clone())
        };
        match kind {
            MediaKind::Video => {
                // Music stops for the film; the video player opens on the group's item.
                if let Some(m) = &music {
                    let p = m.player();
                    if p.is_playing() {
                        self.quiet();
                        p.pause();
                    }
                }
                let open_here = video
                    .as_ref()
                    .is_some_and(|v| v.item_id().as_deref() == Some(entry.item_id.as_str()));
                if open_here {
                    self.align(Some(start));
                } else {
                    {
                        let mut g = self.lock();
                        g.awaiting_ready = true;
                        g.reported_ready = None;
                    }
                    let _ = self.ui_tx.send(UiEvent::Navigate(format!(
                        "/video/play/{}?start={}",
                        item.id,
                        queue.start_position_ticks.round() as i64
                    )));
                }
            }
            MediaKind::Audio => {
                let Some(music) = music else { return };
                if video.as_ref().is_some_and(|v| v.item_id().is_some()) {
                    let _ = self.ui_tx.send(UiEvent::Navigate("/music".into()));
                }
                let (playable, local_index) = playable_of(&queue, &items);
                let (local_ids, current, _) = music.local_queue();
                let new_ids: Vec<String> = playable.iter().map(|t| normalize(&t.id)).collect();
                self.quiet();
                if local_ids != new_ids {
                    music.load(playable, local_index);
                } else if current != local_index as i64 {
                    music.jump(local_index);
                }
                self.align(Some(start));
            }
        }
        let pending = self.lock().pending_command.take();
        if let Some(cmd) = pending {
            if cmd.playlist_item_id == entry.playlist_item_id {
                self.apply_command(cmd);
            }
        }
    }

    fn set_ignore_wait(&self, ignore: bool) {
        {
            let mut g = self.lock();
            if g.ignoring_wait == ignore {
                return;
            }
            g.ignoring_wait = ignore;
        }
        self.send(move |c| async move { sp::set_ignore_wait(&c, ignore).await });
    }

    // ----------------------------------------------------------------------- commands

    fn clear_command_task(&self) {
        if let Some(t) = self.lock().command_task.take() {
            t.abort();
        }
    }

    fn at(&self, local_ms: f64, run: impl FnOnce(&SyncPlayManager) + Send + 'static) {
        self.clear_command_task();
        let delay = local_ms - now_ms();
        if delay <= 4.0 {
            run(self);
            return;
        }
        let me = self.arc();
        let task = runtime().spawn(async move {
            tokio::time::sleep(Duration::from_millis(delay as u64)).await;
            run(&me);
        });
        self.lock().command_task = Some(task);
    }

    pub(super) fn apply_command(&self, cmd: SyncPlayCommand) {
        if cmd.command != CommandKind::Unpause {
            // Unpause is re-sent to whoever reports Ready again; re-applying it is how we resync.
            let key = sp::command_key(&cmd);
            let mut g = self.lock();
            if g.seen.contains(&key) {
                return;
            }
            let excess = g.seen.len().saturating_sub(19);
            g.seen.drain(..excess);
            g.seen.push(key);
        }
        if cmd.command == CommandKind::Stop {
            self.clear_command_task();
            {
                let mut g = self.lock();
                g.allow_play = false;
                g.last_unpause = None;
            }
            self.set_rate(1.0);
            // The Stop a new group sends before anything is shared does not stop what we play.
            if cmd.playlist_item_id == sp::EMPTY_GUID && self.lock().share_on_join {
                return;
            }
            if let Some(p) = self.player() {
                self.quiet();
                p.pause();
            }
            return;
        }
        let entry = playing_entry(&self.state().queue);
        if !entry.is_some_and(|e| e.playlist_item_id == cmd.playlist_item_id) {
            // For a queue we have not applied yet (messages can cross): keep it for then.
            self.lock().pending_command = Some(cmd);
            return;
        }
        let p = self.player();
        let offset = self.time_offset().0;
        let plan = sp::plan_command(&cmd, now_ms(), offset);
        match cmd.command {
            CommandKind::Unpause => {
                {
                    let mut g = self.lock();
                    g.last_unpause = Some((cmd.when, cmd.position_ticks));
                    g.awaiting_ready = false;
                }
                if let Some(p) = &p {
                    if !plan.late
                        && !p.is_playing()
                        && (p.position() - plan.position_seconds).abs() > 0.05
                    {
                        self.quiet();
                        p.seek(plan.position_seconds);
                    }
                }
                let (when, ticks) = (cmd.when, cmd.position_ticks);
                self.at(plan.at_local, move |m| {
                    {
                        let mut g = m.lock();
                        g.allow_play = true;
                        g.grace_until = now_ms() + GRACE_MS;
                    }
                    let Some(cur) = m.player() else { return };
                    let target = sp::expected_position(when, ticks, m.server_now());
                    let off = (cur.position() - target).abs();
                    m.quiet();
                    if off > if cur.is_playing() { 0.3 } else { 0.05 } {
                        cur.seek(target);
                    }
                    if !cur.is_playing() {
                        cur.play();
                    }
                });
            }
            CommandKind::Pause => {
                let position = plan.position_seconds;
                self.at(plan.at_local, move |m| {
                    {
                        let mut g = m.lock();
                        g.allow_play = false;
                        g.last_unpause = None;
                    }
                    m.set_rate(1.0);
                    let Some(cur) = m.player() else { return };
                    m.quiet();
                    cur.pause();
                    if (cur.position() - position).abs() > 0.05 {
                        cur.seek(position);
                    }
                });
            }
            CommandKind::Seek => {
                self.clear_command_task();
                self.align(Some(plan.position_seconds));
            }
            CommandKind::Stop => {}
        }
    }
}

/// The entries this user can access (others are left out of the local queue), and the index of
/// the current one among them.
pub fn playable_of(
    queue: &PlayQueue,
    items: &HashMap<String, BaseItemDto>,
) -> (Vec<BaseItemDto>, usize) {
    let mut tracks = Vec::new();
    let mut index = 0;
    for (i, e) in queue.playlist.iter().enumerate() {
        let Some(it) = items.get(&e.item_id) else { continue };
        if i as i64 == queue.playing_item_index {
            index = tracks.len();
        }
        tracks.push(it.clone());
    }
    (tracks, index)
}

pub fn item_title(item: &BaseItemDto) -> String {
    let name = item.name.clone().unwrap_or_default();
    if item.item_type.as_deref() == Some("Episode") {
        if let Some(series) = &item.series_name {
            return format!("{series} · {name}");
        }
    }
    if item.item_type.as_deref() == Some("Audio") {
        if let Some(artist) = item.artists.first() {
            return format!("{name} · {artist}");
        }
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, name: &str) -> BaseItemDto {
        BaseItemDto {
            id: id.into(),
            name: Some(name.into()),
            ..BaseItemDto::default()
        }
    }

    fn queue(ids: &[&str], playing: i64) -> PlayQueue {
        PlayQueue {
            reason: "NewPlaylist".into(),
            last_update: 0.0,
            playlist: ids
                .iter()
                .map(|i| QueueEntry {
                    item_id: (*i).into(),
                    playlist_item_id: format!("p{i}"),
                })
                .collect(),
            playing_item_index: playing,
            start_position_ticks: 0.0,
            is_playing: false,
            shuffle_mode: String::new(),
            repeat_mode: String::new(),
        }
    }

    #[test]
    fn playing_entry_follows_the_index() {
        let q = Some(queue(&["a", "b"], 1));
        assert_eq!(playing_entry(&q).unwrap().item_id, "b");
        assert!(playing_entry(&Some(queue(&["a"], -1))).is_none());
        assert!(playing_entry(&None).is_none());
    }

    #[test]
    fn playable_skips_inaccessible_items_and_keeps_the_current_index() {
        let q = queue(&["a", "b", "c"], 2);
        let mut items = HashMap::new();
        items.insert("a".to_string(), item("a", "A"));
        items.insert("c".to_string(), item("c", "C"));
        let (tracks, index) = playable_of(&q, &items);
        assert_eq!(tracks.len(), 2);
        assert_eq!(index, 1);
    }

    #[test]
    fn titles_name_the_series_or_the_artist() {
        let mut ep = item("e", "Pilot");
        ep.item_type = Some("Episode".into());
        ep.series_name = Some("Show".into());
        assert_eq!(item_title(&ep), "Show · Pilot");
        let mut song = item("s", "Song");
        song.item_type = Some("Audio".into());
        song.artists = vec!["Band".into()];
        assert_eq!(item_title(&song), "Song · Band");
        assert_eq!(item_title(&item("m", "Film")), "Film");
    }
}
