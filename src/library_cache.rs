//! The account's own lists, kept across restarts so they show at once.
//!
//! Only what belongs to the listener is kept: the playlists in their
//! library, and Home's recently played, top artists and top songs.
//! Recommendations and Spotify's mixes are always read fresh.

use serde::{Deserialize, Serialize};

use crate::api::models::{Artist, PlayHistory, Playlist, Track};

const VERSION: u32 = 1;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Cache {
    version: u32,
    pub account_id: String,
    /// The whole playlist list, saved only once every page has arrived.
    pub playlists: Option<Vec<Playlist>>,
    pub recently_played: Option<Vec<PlayHistory>>,
    pub top_artists: Option<Vec<Artist>>,
    pub top_tracks: Option<Vec<Track>>,
}

impl Cache {
    pub fn new(account_id: String) -> Self {
        Self {
            version: VERSION,
            account_id,
            ..Self::default()
        }
    }

    pub fn valid_for(&self, account: &str) -> bool {
        self.version == VERSION && self.account_id == account
    }

    /// Fills each list this cache lacks from `older`, keeping what is newer.
    pub fn fill_from(&mut self, older: Self) {
        self.playlists = self.playlists.take().or(older.playlists);
        self.recently_played = self.recently_played.take().or(older.recently_played);
        self.top_artists = self.top_artists.take().or(older.top_artists);
        self.top_tracks = self.top_tracks.take().or(older.top_tracks);
    }
}

/// Puts `value` in `slot`, saying whether that changed anything.
pub fn replace<T: Clone + PartialEq>(slot: &mut Option<Vec<T>>, value: &[T]) -> bool {
    if slot.as_deref() == Some(value) {
        return false;
    }
    *slot = Some(value.to_vec());
    true
}

pub async fn read(path: &std::path::Path, account: &str) -> Option<Cache> {
    let bytes = tokio::fs::read(path).await.ok()?;
    let cache: Cache = serde_json::from_slice(&bytes).ok()?;
    cache.valid_for(account).then_some(cache)
}

pub async fn write(path: &std::path::Path, cache: &Cache) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let bytes = serde_json::to_vec(cache).map_err(std::io::Error::other)?;
    let temporary = path.with_extension("json.tmp");
    tokio::fs::write(&temporary, bytes).await?;
    crate::util::replace_file(&temporary, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playlist(id: &str) -> Playlist {
        Playlist {
            id: id.into(),
            ..Playlist::default()
        }
    }

    #[test]
    fn a_cache_is_only_for_its_account_and_version() {
        let cache = Cache::new("alice".into());
        assert!(cache.valid_for("alice"));
        assert!(!cache.valid_for("bob"));
        let old: Cache = serde_json::from_str(r#"{"account_id":"alice"}"#).unwrap();
        assert!(
            !old.valid_for("alice"),
            "an unversioned file is not trusted"
        );
    }

    #[test]
    fn newer_lists_win_over_the_file_they_fill_from() {
        let mut live = Cache::new("alice".into());
        live.playlists = Some(vec![playlist("new")]);
        let mut file = Cache::new("alice".into());
        file.playlists = Some(vec![playlist("old")]);
        file.top_artists = Some(vec![Artist::default()]);
        live.fill_from(file);
        assert_eq!(live.playlists, Some(vec![playlist("new")]));
        assert_eq!(live.top_artists, Some(vec![Artist::default()]));
    }

    #[test]
    fn replacing_with_the_same_list_changes_nothing() {
        let mut slot = None;
        assert!(replace(&mut slot, &[playlist("a")]));
        assert!(!replace(&mut slot, &[playlist("a")]));
        assert!(replace(&mut slot, &[playlist("b")]));
    }
}
