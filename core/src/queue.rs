//! The play queue: what's playing, what's next, shuffle and repeat. Pure logic (no sound), so the
//! Windows and iPhone apps behave the same and it's all tested here.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Repeat {
    #[default]
    Off,
    /// start the queue again after the last song
    All,
    /// play the same song again and again
    One,
}

impl Repeat {
    /// Off -> All -> One -> Off (the repeat button)
    pub fn cycle(self) -> Repeat {
        match self {
            Repeat::Off => Repeat::All,
            Repeat::All => Repeat::One,
            Repeat::One => Repeat::Off,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Queue {
    /// songs in the order they'll play (shuffled when shuffle is on)
    order: Vec<String>,
    /// the same songs in their own order, to go back to when shuffle is turned off
    original: Vec<String>,
    pos: Option<usize>,
    shuffle: bool,
    repeat: Repeat,
    /// where the queue came from ("Blue Hours", "Search"), for Now Playing
    pub source: String,
    /// how many of the songs right after the current one were put there by hand (Play next / Add to queue): the
    /// website's "Next in queue", shown before what follows from the album or playlist
    user_q: usize,
}

impl Queue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Play a list (an album, search results…) starting at `start`. With shuffle on, the chosen
    /// song plays first and the rest follow in random order.
    pub fn play(&mut self, ids: Vec<String>, start: usize, source: &str) {
        self.source = source.to_string();
        self.user_q = 0;
        self.original = ids.clone();
        self.order = ids;
        if self.order.is_empty() {
            self.pos = None;
            return;
        }
        let start = start.min(self.order.len() - 1);
        self.pos = Some(start);
        if self.shuffle {
            self.shuffle_rest();
        }
    }

    /// Play a list shuffled from a random song (the shuffle button on an album).
    pub fn play_shuffled(&mut self, ids: Vec<String>, source: &str) {
        self.shuffle = true;
        let start = if ids.is_empty() { 0 } else { fastrand::usize(..ids.len()) };
        self.play(ids, start, source);
    }

    fn shuffle_rest(&mut self) {
        let Some(p) = self.pos else { return };
        let current = self.order[p].clone();
        let mut rest: Vec<String> = self.order.iter().enumerate().filter(|(i, _)| *i != p).map(|(_, s)| s.clone()).collect();
        fastrand::shuffle(&mut rest);
        self.order = std::iter::once(current).chain(rest).collect();
        self.pos = Some(0);
    }

    pub fn current(&self) -> Option<&str> {
        self.pos.map(|p| self.order[p].as_str())
    }
    pub fn position(&self) -> Option<usize> {
        self.pos
    }
    pub fn items(&self) -> &[String] {
        &self.order
    }
    pub fn len(&self) -> usize {
        self.order.len()
    }
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }
    /// what plays after the current song
    pub fn upcoming(&self) -> &[String] {
        match self.pos {
            Some(p) => &self.order[p + 1..],
            None => &self.order[..],
        }
    }
    pub fn shuffle(&self) -> bool {
        self.shuffle
    }
    /// How many of the upcoming songs you queued yourself (they come first).
    pub fn user_queued(&self) -> usize {
        self.user_q.min(self.upcoming().len())
    }
    pub fn repeat(&self) -> Repeat {
        self.repeat
    }
    pub fn set_repeat(&mut self, r: Repeat) {
        self.repeat = r;
    }

    pub fn set_shuffle(&mut self, on: bool) {
        if on == self.shuffle {
            return;
        }
        self.shuffle = on;
        self.user_q = 0;
        if on {
            self.shuffle_rest();
        } else {
            // back to the list's own order, still on the same song
            let current = self.current().map(str::to_string);
            self.order = self.original.clone();
            self.pos = current.and_then(|c| self.order.iter().position(|s| *s == c)).or(if self.order.is_empty() { None } else { Some(0) });
        }
    }

    /// The song finished (`auto`) or Next was pressed. Returns the song to play now, or `None`
    /// when the queue has ended (the last song stays selected, ready to play again).
    pub fn next(&mut self, auto: bool) -> Option<&str> {
        let p = self.pos?;
        if auto && self.repeat == Repeat::One {
            return self.current();
        }
        if p + 1 < self.order.len() {
            self.pos = Some(p + 1);
            self.user_q = self.user_q.saturating_sub(1); // one of your own picks has just been reached
        } else if self.repeat != Repeat::Off {
            self.user_q = 0;
            if self.shuffle && self.order.len() > 2 {
                // a fresh shuffle each time round
                let last = self.order[p].clone();
                fastrand::shuffle(&mut self.order);
                if self.order[0] == last {
                    let n = self.order.len();
                    self.order.swap(0, n - 1);
                }
            }
            self.pos = Some(0);
        } else {
            return None;
        }
        self.current()
    }

    /// Previous song (the app restarts the current song instead when it's a few seconds in).
    pub fn previous(&mut self) -> Option<&str> {
        let p = self.pos?;
        self.user_q = 0;
        if p > 0 {
            self.pos = Some(p - 1);
        } else if self.repeat == Repeat::All {
            self.pos = Some(self.order.len() - 1);
        }
        self.current()
    }

    /// Jump to a song in the queue (clicked in the queue list).
    pub fn jump(&mut self, index: usize) -> Option<&str> {
        if index < self.order.len() {
            // your picks that are still ahead stay yours
            let end = self.pos.map(|p| p + self.user_q).unwrap_or(0);
            self.user_q = if self.pos.map(|p| index > p).unwrap_or(false) && index <= end { end - index } else { 0 };
            self.pos = Some(index);
        }
        self.current()
    }

    /// "Play next": right after the current song.
    pub fn play_next(&mut self, id: &str) {
        let at = self.pos.map(|p| p + 1).unwrap_or(0);
        self.order.insert(at, id.to_string());
        if self.pos.is_some() {
            self.user_q += 1;
        }
        let o_at = self.current_original_index().map(|i| i + 1).unwrap_or(self.original.len());
        self.original.insert(o_at.min(self.original.len()), id.to_string());
        if self.pos.is_none() {
            self.pos = Some(0);
        }
    }

    /// "Add to queue": after the songs you already queued (before what follows from the album or playlist), like the website.
    pub fn add(&mut self, id: &str) {
        match self.pos {
            Some(p) => {
                let at = (p + 1 + self.user_q).min(self.order.len());
                self.order.insert(at, id.to_string());
                self.user_q += 1;
            }
            None => self.order.push(id.to_string()),
        }
        // the list's own order follows when shuffle is off
        if self.shuffle {
            self.original.push(id.to_string());
        } else {
            self.original = self.order.clone();
        }
        if self.pos.is_none() {
            self.pos = Some(0);
        }
    }

    /// Take a song out of the queue (not the one playing).
    pub fn remove(&mut self, index: usize) {
        let Some(p) = self.pos else { return };
        if index >= self.order.len() || index == p {
            return;
        }
        if index > p && index <= p + self.user_q {
            self.user_q -= 1;
        }
        let id = self.order.remove(index);
        if let Some(i) = self.original.iter().position(|s| *s == id) {
            self.original.remove(i);
        }
        if index < p {
            self.pos = Some(p - 1);
        }
    }

    /// Drag a song to another place in the queue (the playing song can move too and keeps playing).
    pub fn move_item(&mut self, from: usize, to: usize) {
        if from >= self.order.len() || to >= self.order.len() || from == to {
            return;
        }
        let current = self.pos;
        // moving a song into or out of your own picks (the ones right after the current song)
        if let Some(p) = current {
            let mine = |i: usize| i > p && i <= p + self.user_q;
            match (mine(from), mine(to)) {
                (true, false) => self.user_q -= 1,
                (false, true) => self.user_q += 1,
                _ => {}
            }
        }
        let item = self.order.remove(from);
        self.order.insert(to, item.clone());
        self.pos = current.map(|p| {
            if p == from {
                to
            } else if from < p && to >= p {
                p - 1
            } else if from > p && to <= p {
                p + 1
            } else {
                p
            }
        });
        // the list's own order follows when shuffle is off (so turning shuffle on and off keeps the change)
        if !self.shuffle {
            self.original = self.order.clone();
        }
    }

    /// Show another device's queue as it is (no shuffling): the song at `index` is the current one.
    pub fn mirror(&mut self, ids: Vec<String>, index: usize, source: &str) {
        self.source = source.to_string();
        self.user_q = 0;
        self.original = ids.clone();
        self.order = ids;
        self.pos = if self.order.is_empty() { None } else { Some(index.min(self.order.len() - 1)) };
    }

    /// Take everything after the current song out of the queue.
    pub fn clear_upcoming(&mut self) {
        match self.pos {
            Some(p) => {
                self.order.truncate(p + 1);
                self.original = self.order.clone();
                self.user_q = 0;
            }
            None => self.clear(),
        }
    }

    pub fn clear(&mut self) {
        *self = Queue { shuffle: self.shuffle, repeat: self.repeat, ..Default::default() };
    }

    fn current_original_index(&self) -> Option<usize> {
        let c = self.current()?;
        self.original.iter().position(|s| s == c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(n: usize) -> Vec<String> {
        (1..=n).map(|i| format!("t{i}")).collect()
    }

    #[test]
    fn plays_through_and_stops() {
        let mut q = Queue::new();
        q.play(ids(3), 1, "Album");
        assert_eq!(q.current(), Some("t2"));
        assert_eq!(q.next(true), Some("t3"));
        assert_eq!(q.next(true), None);
        assert_eq!(q.current(), Some("t3"));
        assert_eq!(q.previous(), Some("t2"));
        assert_eq!(q.upcoming(), &["t3".to_string()]);
    }

    #[test]
    fn repeat_modes() {
        let mut q = Queue::new();
        q.play(ids(2), 0, "");
        q.set_repeat(Repeat::One);
        assert_eq!(q.next(true), Some("t1"), "repeat one replays when a song ends");
        assert_eq!(q.next(false), Some("t2"), "but Next still moves on");
        q.set_repeat(Repeat::All);
        assert_eq!(q.next(true), Some("t1"));
        assert_eq!(q.previous(), Some("t2"), "previous from the first wraps with repeat all");
        assert_eq!(Repeat::Off.cycle().cycle().cycle(), Repeat::Off);
    }

    #[test]
    fn shuffle_keeps_current_first_and_unshuffles() {
        let mut q = Queue::new();
        q.play(ids(20), 4, "");
        q.set_shuffle(true);
        assert_eq!(q.current(), Some("t5"));
        assert_eq!(q.position(), Some(0));
        let mut all = q.items().to_vec();
        all.sort();
        let mut want = ids(20);
        want.sort();
        assert_eq!(all, want, "same songs, none lost or doubled");
        q.next(false);
        let now = q.current().unwrap().to_string();
        q.set_shuffle(false);
        assert_eq!(q.current(), Some(now.as_str()), "still on the same song");
        assert_eq!(q.items(), ids(20).as_slice(), "back in album order");
    }

    #[test]
    fn play_shuffled_starts_anywhere() {
        let mut q = Queue::new();
        q.play_shuffled(ids(5), "");
        assert!(q.shuffle());
        assert_eq!(q.position(), Some(0));
        assert_eq!(q.len(), 5);
    }

    #[test]
    fn play_next_add_remove() {
        let mut q = Queue::new();
        q.play(ids(3), 0, "");
        q.play_next("x");
        q.add("y");
        // your own picks come right after the current song, in the order you queued them (like the website)
        assert_eq!(q.items(), &["t1", "x", "y", "t2", "t3"].map(String::from));
        assert_eq!(q.user_queued(), 2);
        q.remove(0); // the playing song can't be removed
        assert_eq!(q.len(), 5);
        q.jump(4);
        assert_eq!(q.user_queued(), 0, "jumping past your picks leaves none ahead");
        q.remove(1);
        assert_eq!(q.current(), Some("t3"));
        assert_eq!(q.position(), Some(3));
        q.set_shuffle(true);
        q.set_shuffle(false);
        assert_eq!(q.items(), &["t1", "y", "t2", "t3"].map(String::from), "removed and added songs carry over to the unshuffled order");

        // reaching one of your picks counts it off; moving a song in or out of them changes the count
        let mut q = Queue::new();
        q.play(ids(3), 0, "");
        q.add("a");
        q.add("b");
        assert_eq!(q.user_queued(), 2);
        q.next(false);
        assert_eq!((q.current(), q.user_queued()), (Some("a"), 1));
        q.move_item(2, 4); // "b" dragged below the album's songs
        assert_eq!(q.user_queued(), 0);
        q.move_item(4, 2); // back up, but there are no picks left for it to join (the website counts the same way)
        assert_eq!(q.user_queued(), 0);
    }

    #[test]
    fn drag_to_reorder() {
        let mut q = Queue::new();
        q.play(ids(5), 2, "");
        q.move_item(4, 0); // t5 to the top: the playing t3 shifts down one
        assert_eq!(q.items(), &["t5", "t1", "t2", "t3", "t4"].map(String::from));
        assert_eq!(q.current(), Some("t3"));
        q.move_item(3, 4); // the playing song itself
        assert_eq!(q.current(), Some("t3"));
        assert_eq!(q.position(), Some(4));
        q.move_item(0, 2);
        assert_eq!(q.current(), Some("t3"));
        q.move_item(9, 0); // out of range: nothing
        assert_eq!(q.len(), 5);
        q.set_shuffle(true);
        q.set_shuffle(false);
        assert_eq!(q.items(), &["t1", "t2", "t5", "t4", "t3"].map(String::from), "the new order is kept");
    }

    #[test]
    fn add_to_empty_queue_selects_it() {
        let mut q = Queue::new();
        assert_eq!(q.next(true), None);
        q.add("a");
        assert_eq!(q.current(), Some("a"));
    }
}
