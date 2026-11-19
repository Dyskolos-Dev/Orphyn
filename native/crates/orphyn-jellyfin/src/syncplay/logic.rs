//! Pure SyncPlay decisions (no I/O): when and where a group command lands on this computer, how to
//! correct drift while playing, and what can be told about the other members.
use super::protocol::{CommandKind, GroupState, QueueEntry, SyncPlayCommand};
use crate::TICKS_PER_SECOND;

// ------------------------------------------------------------------------- scheduling

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CommandPlan {
    /// Local time (ms) at which to act.
    pub at_local: f64,
    /// Position (s) to be at when acting (for Unpause: where playback starts at `at_local`).
    pub position_seconds: f64,
    /// The command's moment has already passed (we act at once, adjusted for the lateness).
    pub late: bool,
}

/// Where and when a command applies here. `offset_ms` is server − local (see the estimator).
/// - Unpause: start at `When` from `PositionTicks`; when late, start now, further in.
/// - Pause: stop at `When`, then sit on `PositionTicks` (where everyone stops).
/// - Seek / Stop: now.
pub fn plan_command(cmd: &SyncPlayCommand, now_local: f64, offset_ms: f64) -> CommandPlan {
    let when_local = cmd.when - offset_ms;
    let position = cmd.position_ticks / TICKS_PER_SECOND;
    match cmd.command {
        CommandKind::Unpause => {
            if when_local >= now_local {
                CommandPlan {
                    at_local: when_local,
                    position_seconds: position,
                    late: false,
                }
            } else {
                CommandPlan {
                    at_local: now_local,
                    position_seconds: position + (now_local - when_local) / 1000.0,
                    late: true,
                }
            }
        }
        CommandKind::Pause => CommandPlan {
            at_local: when_local.max(now_local),
            position_seconds: position,
            late: when_local < now_local,
        },
        _ => CommandPlan {
            at_local: now_local,
            position_seconds: position,
            late: when_local < now_local,
        },
    }
}

/// The server repeats a command to whoever reports Ready again: apply each one once.
pub fn command_key(cmd: &SyncPlayCommand) -> String {
    format!(
        "{:?}|{}|{}|{}",
        cmd.command, cmd.playlist_item_id, cmd.when, cmd.position_ticks
    )
}

/// Where the group is now (s), given the last Unpause and the server clock.
pub fn expected_position(last_unpause_when: f64, last_unpause_ticks: f64, server_now: f64) -> f64 {
    last_unpause_ticks / TICKS_PER_SECOND + (server_now - last_unpause_when).max(0.0) / 1000.0
}

// ------------------------------------------------------------------------- drift correction

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DriftAction {
    None,
    Rate(f64),
    Seek,
}

/// Below this, playback is in sync: normal speed.
pub const DEAD_BAND_S: f64 = 0.04;
/// Up to this, a player that can change speed nudges it; beyond, it seeks.
pub const NUDGE_LIMIT_S: f64 = 1.0;
/// A player without speed control leaves this much alone, then seeks.
pub const SEEK_ONLY_THRESHOLD_S: f64 = 0.5;
pub const MIN_RATE: f64 = 0.95;
pub const MAX_RATE: f64 = 1.05;
/// The nudge aims to absorb the drift over about this long (s).
pub const CORRECTION_WINDOW_S: f64 = 2.0;

/// `drift_s` = local position − expected position (positive: we are ahead). Small drift → a gentle
/// rate change (0.95–1.05) that absorbs it in a few seconds; large drift → a seek.
pub fn drift_action(drift_s: f64, can_nudge: bool) -> DriftAction {
    if !drift_s.is_finite() {
        return DriftAction::None;
    }
    let abs = drift_s.abs();
    if !can_nudge {
        return if abs > SEEK_ONLY_THRESHOLD_S {
            DriftAction::Seek
        } else {
            DriftAction::None
        };
    }
    if abs <= DEAD_BAND_S {
        return DriftAction::Rate(1.0);
    }
    if abs > NUDGE_LIMIT_S {
        return DriftAction::Seek;
    }
    let rate = (1.0 - drift_s / CORRECTION_WINDOW_S).clamp(MIN_RATE, MAX_RATE);
    DriftAction::Rate((rate * 1000.0).round() / 1000.0)
}

// ------------------------------------------------------------------------- queue

/// Same items in the same order (the local queue already matches the group's).
pub fn same_items(local: &[String], group: &[QueueEntry]) -> bool {
    local.len() == group.len() && local.iter().zip(group).all(|(a, b)| *a == b.item_id)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueMove {
    Next,
    Previous,
    Jump,
    None,
}

/// What a local change of the current entry means for the group.
pub fn queue_move(from: usize, to: usize, length: usize) -> QueueMove {
    if from == to {
        QueueMove::None
    } else if to == from + 1 || (from + 1 == length && to == 0) {
        QueueMove::Next
    } else if to + 1 == from {
        QueueMove::Previous
    } else {
        QueueMove::Jump
    }
}

// ------------------------------------------------------------------------- members

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberState {
    Ready,
    Buffering,
    Waiting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub name: String,
    pub is_self: bool,
    pub state: MemberState,
}

/// The server only gives the members' names and the group's state, not who is buffering. What we
/// do know: our own state; when the group waits while we are ready and only one other person is
/// there, it is that person; otherwise the others are shown as "waiting" without a name.
pub fn describe_members(
    participants: &[String],
    self_name: &str,
    self_ready: bool,
    group_state: GroupState,
) -> Vec<Member> {
    let mut self_seen = false;
    let others: Vec<&String> = participants
        .iter()
        .filter(|p| {
            if !self_seen && p.as_str() == self_name {
                self_seen = true;
                false
            } else {
                true
            }
        })
        .collect();
    let others_state = if group_state == GroupState::Waiting {
        if self_ready && others.len() == 1 {
            MemberState::Buffering
        } else {
            MemberState::Waiting
        }
    } else {
        MemberState::Ready
    };
    let mut out = vec![Member {
        name: self_name.to_string(),
        is_self: true,
        state: if self_ready {
            MemberState::Ready
        } else {
            MemberState::Buffering
        },
    }];
    out.extend(others.into_iter().map(|n| Member {
        name: n.clone(),
        is_self: false,
        state: others_state,
    }));
    out
}

/// Who the group waits for, when we can tell (we are ready and there is one other member).
pub fn waiting_for(members: &[Member]) -> Option<String> {
    let mut it = members
        .iter()
        .filter(|m| !m.is_self && m.state == MemberState::Buffering);
    match (it.next(), it.next()) {
        (Some(m), None) => Some(m.name.clone()),
        _ => None,
    }
}

/// Who issued a command we did not send ourselves: only knowable with exactly one other member.
pub fn likely_author(participants: &[String], self_name: &str) -> Option<String> {
    let mut others = participants.iter().filter(|p| p.as_str() != self_name);
    match (others.next(), others.next()) {
        (Some(o), None) => Some(o.clone()),
        _ => None,
    }
}

/// Two-letter initials for an avatar.
pub fn member_initials(name: &str) -> String {
    let mut parts = name
        .trim()
        .split(|c: char| c.is_whitespace() || "._-".contains(c))
        .filter(|s| !s.is_empty());
    let Some(first) = parts.next() else {
        return "?".into();
    };
    match parts.next() {
        None => first.chars().take(2).collect::<String>().to_uppercase(),
        Some(second) => format!(
            "{}{}",
            first.chars().next().unwrap_or(' '),
            second.chars().next().unwrap_or(' ')
        )
        .to_uppercase(),
    }
}

pub const AVATAR_COLORS: [&str; 8] = [
    "blue", "teal", "green", "orange", "pink", "purple", "indigo", "red",
];

/// Stable system colour name for a member's avatar, from their name.
pub fn member_color(name: &str) -> &'static str {
    let mut h: u32 = 0;
    for unit in name.encode_utf16() {
        h = h.wrapping_mul(31).wrapping_add(u32::from(unit));
    }
    AVATAR_COLORS[h as usize % AVATAR_COLORS.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(kind: CommandKind, when: f64, ticks: f64) -> SyncPlayCommand {
        SyncPlayCommand {
            group_id: "g".into(),
            playlist_item_id: "p".into(),
            when,
            position_ticks: ticks,
            command: kind,
            emitted_at: when,
        }
    }

    #[test]
    fn plans_unpause_in_the_future_and_late() {
        // Server 1000 ms ahead of local: When 5000 server = 4000 local.
        let p = plan_command(
            &cmd(CommandKind::Unpause, 5000.0, 20_000_000.0),
            3000.0,
            1000.0,
        );
        assert_eq!(
            p,
            CommandPlan {
                at_local: 4000.0,
                position_seconds: 2.0,
                late: false
            }
        );
        let late = plan_command(
            &cmd(CommandKind::Unpause, 5000.0, 20_000_000.0),
            4500.0,
            1000.0,
        );
        assert!(late.late);
        assert_eq!(late.at_local, 4500.0);
        assert!((late.position_seconds - 2.5).abs() < 1e-9);
    }

    #[test]
    fn plans_pause_seek_stop() {
        let p = plan_command(&cmd(CommandKind::Pause, 5000.0, 10_000_000.0), 6000.0, 0.0);
        assert_eq!(
            (p.at_local, p.position_seconds, p.late),
            (6000.0, 1.0, true)
        );
        let s = plan_command(&cmd(CommandKind::Seek, 5000.0, 30_000_000.0), 1000.0, 0.0);
        assert_eq!(
            (s.at_local, s.position_seconds, s.late),
            (1000.0, 3.0, false)
        );
    }

    #[test]
    fn command_keys_differ_by_command_and_time() {
        let a = cmd(CommandKind::Pause, 1.0, 2.0);
        assert_eq!(command_key(&a), command_key(&a.clone()));
        assert_ne!(
            command_key(&a),
            command_key(&cmd(CommandKind::Unpause, 1.0, 2.0))
        );
    }

    #[test]
    fn expected_position_advances_with_server_time() {
        assert_eq!(expected_position(1000.0, 10_000_000.0, 3000.0), 3.0);
        assert_eq!(expected_position(1000.0, 10_000_000.0, 500.0), 1.0);
    }

    #[test]
    fn drift_actions() {
        assert_eq!(drift_action(f64::NAN, true), DriftAction::None);
        assert_eq!(drift_action(0.02, true), DriftAction::Rate(1.0));
        assert_eq!(drift_action(0.3, false), DriftAction::None);
        assert_eq!(drift_action(0.6, false), DriftAction::Seek);
        assert_eq!(drift_action(1.5, true), DriftAction::Seek);
        assert_eq!(drift_action(0.2, true), DriftAction::Rate(0.95));
        assert_eq!(drift_action(-0.2, true), DriftAction::Rate(1.05));
        match drift_action(0.05, true) {
            DriftAction::Rate(r) => assert!((0.95..1.0).contains(&r)),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn queue_moves() {
        assert_eq!(queue_move(1, 1, 4), QueueMove::None);
        assert_eq!(queue_move(1, 2, 4), QueueMove::Next);
        assert_eq!(queue_move(3, 0, 4), QueueMove::Next);
        assert_eq!(queue_move(2, 1, 4), QueueMove::Previous);
        assert_eq!(queue_move(0, 3, 4), QueueMove::Jump);
        let group = vec![QueueEntry {
            item_id: "a".into(),
            playlist_item_id: "1".into(),
        }];
        assert!(same_items(&["a".into()], &group));
        assert!(!same_items(&["b".into()], &group));
    }

    #[test]
    fn members_and_authorship() {
        let names = vec!["me".to_string(), "bob".to_string()];
        let m = describe_members(&names, "me", true, GroupState::Waiting);
        assert_eq!(m[1].state, MemberState::Buffering);
        assert_eq!(waiting_for(&m).as_deref(), Some("bob"));
        let m = describe_members(
            &["me".into(), "a".into(), "b".into()],
            "me",
            true,
            GroupState::Waiting,
        );
        assert_eq!(m[1].state, MemberState::Waiting);
        assert_eq!(waiting_for(&m), None);
        let m = describe_members(&names, "me", false, GroupState::Playing);
        assert_eq!(
            (m[0].state, m[1].state),
            (MemberState::Buffering, MemberState::Ready)
        );
        assert_eq!(likely_author(&names, "me").as_deref(), Some("bob"));
        assert_eq!(likely_author(&["me".into()], "me"), None);
    }

    #[test]
    fn initials_and_colours_are_stable() {
        assert_eq!(member_initials("Jean-Luc Picard"), "JL");
        assert_eq!(member_initials("alice"), "AL");
        assert_eq!(member_initials("  "), "?");
        assert_eq!(member_color("alice"), member_color("alice"));
    }
}
