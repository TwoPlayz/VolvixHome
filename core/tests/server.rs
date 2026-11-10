//! Tests against a real local server. Start one first:
//!
//!     powershell -File dev.ps1 -Mocks            (port 4747; or -Port 4790 and set VH_TEST_SERVER)
//!     cargo test -- --ignored
//!
//! On a fresh dev server (.dev deleted) the first run creates the admin account below. These are
//! dev-only values for the throwaway .dev server, never a real account.
use std::io::{Read, Seek, SeekFrom};

use volvix_core::covers::CoverCache;
use volvix_core::stream::HttpFile;
use volvix_core::*;

const DEV_NAME: &str = "Dev Admin";
const DEV_PASSWORD: &str = "dev-password-123";

fn server() -> String {
    std::env::var("VH_TEST_SERVER").unwrap_or_else(|_| "localhost:4747".into())
}

fn signed_in() -> (Client, Library) {
    let (c, status) = Client::connect(&server()).expect("start the dev server first (see the top of this file)");
    let (c, who) = if status.setup_needed { c.setup(DEV_NAME, DEV_PASSWORD) } else { c.login(DEV_NAME, DEV_PASSWORD) }.expect("sign in");
    assert!(!who.token.is_empty());
    let lib = Library::new(c.library().expect("library"));
    (c, lib)
}

#[test]
#[ignore]
fn connect_and_sign_in() {
    let (c, lib) = signed_in();
    let me = c.me().unwrap();
    assert_eq!(me.user.name, DEV_NAME);
    assert!(!lib.is_empty(), "the dev server makes sample music");
    // a wrong password is the server's own message
    let bad = Client::new(c.base(), None).login(DEV_NAME, "nope-nope-nope");
    match bad {
        Err(Error::Server { status: 401, message }) => assert!(message.contains("incorrect"), "{message}"),
        other => panic!("expected the server's wrong-password message, got {:?}", other.map(|_| ())),
    }
    // without a token: signed out
    assert_eq!(Client::new(c.base(), None).library().unwrap_err(), Error::SignedOut);
}

#[test]
#[ignore]
fn library_lookups_and_search() {
    let (_, lib) = signed_in();
    let album = &lib.albums()[0];
    let tracks = lib.album_tracks(&album.id);
    assert_eq!(tracks.len(), album.track_ids.len());
    let r = search::search(&lib, &[], &album.title);
    assert!(r.albums.iter().any(|a| a.id == album.id));
}

#[test]
#[ignore]
fn user_data_round_trip() {
    let (c, _) = signed_in();
    let mut d = c.user_data().unwrap().unwrap_or_default();
    d.rest.insert("nativeTest".into(), serde_json::json!(true));
    c.save_user_data(&d).unwrap();
    let back = c.user_data().unwrap().unwrap();
    assert_eq!(back.rest["nativeTest"], true);
}

#[test]
#[ignore]
fn streaming_with_seek() {
    let (c, lib) = signed_in();
    let t = &lib.tracks()[0];
    let mut f = HttpFile::open(&c, &t.id).unwrap();
    let len = f.len().expect("size known");
    let mut head = [0u8; 16];
    f.read_exact(&mut head).unwrap();
    // jump near the end (a new range request) and read the rest
    f.seek(SeekFrom::Start(len - 100)).unwrap();
    let mut tail = vec![];
    f.read_to_end(&mut tail).unwrap();
    assert_eq!(tail.len(), 100);
    // back to the start: same bytes as before
    f.seek(SeekFrom::Start(0)).unwrap();
    let mut again = [0u8; 16];
    f.read_exact(&mut again).unwrap();
    assert_eq!(head, again);
}

#[test]
#[ignore]
fn covers_are_cached() {
    let (c, lib) = signed_in();
    let Some(a) = lib.albums().iter().find(|a| a.cover && a.cover_v > 0) else { return };
    let dir = std::env::temp_dir().join(format!("vh-cover-test-{}", std::process::id()));
    let cache = CoverCache::new(&dir);
    let first = cache.get(&c, &a.id, 640, a.cover_v).unwrap();
    assert!(!first.is_empty());
    assert_eq!(cache.cached(&a.id, 640, a.cover_v).unwrap(), first);
    let _ = std::fs::remove_dir_all(&dir);
}
