//! Every playlist you can see, in one shape: your own (in your app data), collaborative ones (on
//! the server) and other people's public ones (from the Household page).
use crate::model::{Household, Playlist, SharedPlaylist, UserData};
use crate::userdata::{LIKED_ID, ME};

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// Liked Songs
    Liked,
    /// yours, only in your data
    Own,
    /// collaborative, on the server; `mine` = you own it
    Shared { mine: bool },
    /// someone else's public playlist (read-only); `owner` = their name
    Household { owner: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlaylistView {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: Kind,
    pub public: bool,
    pub tracks: Vec<String>,
    /// server ids of the people who can edit (collaborative only)
    pub collaborators: Vec<String>,
    pub invited: Vec<String>,
}

impl PlaylistView {
    /// You may add, remove and reorder songs.
    pub fn can_edit(&self) -> bool {
        !matches!(self.kind, Kind::Household { .. })
    }
    /// You may rename or delete it.
    pub fn can_manage(&self) -> bool {
        matches!(self.kind, Kind::Own | Kind::Shared { mine: true })
    }
    pub fn is_shared(&self) -> bool {
        matches!(self.kind, Kind::Shared { .. })
    }
}

fn own(p: &Playlist) -> PlaylistView {
    PlaylistView {
        id: p.id.clone(),
        name: p.name.clone(),
        description: p.description.clone(),
        kind: if p.liked || p.id == LIKED_ID { Kind::Liked } else { Kind::Own },
        public: p.public,
        tracks: p.track_ids(),
        collaborators: vec![],
        invited: vec![],
    }
}

fn shared(p: &SharedPlaylist, me: &str, kind: Option<Kind>) -> PlaylistView {
    PlaylistView {
        id: p.id.clone(),
        name: p.name.clone(),
        description: p.description.clone(),
        kind: kind.unwrap_or(Kind::Shared { mine: p.owner == me || p.owner == ME }),
        public: p.public,
        tracks: p.tracks.iter().map(|t| t.id.clone()).collect(),
        collaborators: p.collaborators.clone(),
        invited: p.invited.clone(),
    }
}

/// Yours first (Liked Songs on top, then your own, then collaborative ones), then the household's.
pub fn all(data: &UserData, shared_list: &[SharedPlaylist], household: &Household, me: &str) -> Vec<PlaylistView> {
    let mut out: Vec<PlaylistView> = data.playlists.iter().map(own).collect();
    out.extend(shared_list.iter().map(|p| shared(p, me, None)));
    for m in &household.members {
        for p in &m.playlists {
            if out.iter().any(|x| x.id == p.id) {
                continue; // a collaborative one you're on, already listed
            }
            out.push(shared(p, me, Some(Kind::Household { owner: m.name.clone() })));
        }
    }
    out
}

/// Just the ones you can add songs to.
pub fn editable(list: &[PlaylistView]) -> Vec<&PlaylistView> {
    list.iter().filter(|p| p.can_edit()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Member, PlaylistEntry};

    #[test]
    fn combined() {
        let data = UserData {
            playlists: vec![
                Playlist { id: LIKED_ID.into(), name: "Liked Songs".into(), liked: true, ..Default::default() },
                Playlist { id: "p1".into(), name: "Mine".into(), tracks: vec![PlaylistEntry { id: "t1".into(), ..Default::default() }], ..Default::default() },
            ],
            ..Default::default()
        };
        let sh = vec![SharedPlaylist { id: "s1".into(), owner: "u9".into(), ..Default::default() }];
        let hh = Household {
            members: vec![Member {
                id: "u9".into(),
                name: "Sam".into(),
                playlists: vec![SharedPlaylist { id: "s1".into(), ..Default::default() }, SharedPlaylist { id: "h1".into(), name: "Sam's".into(), ..Default::default() }],
                ..Default::default()
            }],
        };
        let l = all(&data, &sh, &hh, "u1");
        assert_eq!(l.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), vec![LIKED_ID, "p1", "s1", "h1"]);
        assert_eq!(l[0].kind, Kind::Liked);
        assert_eq!(l[1].tracks, vec!["t1"]);
        assert_eq!(l[2].kind, Kind::Shared { mine: false });
        assert!(l[2].can_edit() && !l[2].can_manage());
        assert_eq!(l[3].kind, Kind::Household { owner: "Sam".into() });
        assert_eq!(editable(&l).len(), 3);
    }
}
