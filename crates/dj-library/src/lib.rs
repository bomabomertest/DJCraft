//! Track library: metadata, search, crates / sub-crates / smart crates, history, prepare list,
//! play counts, ratings, colours. In-memory; persistence (SQLite) is a later step.

pub type TrackId = u32;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrackMeta {
    pub id: TrackId,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub bpm: Option<f32>,
    pub key: Option<String>,
    pub duration_s: f32,
    /// 0..=5 stars
    pub rating: u8,
    pub plays: u32,
    pub color: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Rule {
    BpmBetween(f32, f32),
    KeyIs(String),
    RatingAtLeast(u8),
    PlaysAtLeast(u32),
    TextContains(String),
}

impl Rule {
    pub fn matches(&self, t: &TrackMeta) -> bool {
        match self {
            Rule::BpmBetween(lo, hi) => t.bpm.map_or(false, |b| b >= *lo && b <= *hi),
            Rule::KeyIs(k) => t.key.as_deref().map_or(false, |x| x.eq_ignore_ascii_case(k)),
            Rule::RatingAtLeast(r) => t.rating >= *r,
            Rule::PlaysAtLeast(p) => t.plays >= *p,
            Rule::TextContains(s) => text_match(t, s),
        }
    }
}

fn text_match(t: &TrackMeta, query: &str) -> bool {
    let hay = format!("{} {} {}", t.title, t.artist, t.path).to_lowercase();
    query.split_whitespace().all(|w| hay.contains(&w.to_lowercase()))
}

#[derive(Clone, Debug, Default)]
pub struct Crate {
    pub name: String,
    pub color: Option<u32>,
    pub favorite: bool,
    pub tracks: Vec<TrackId>,
    pub children: Vec<Crate>,
    /// If set, this is a smart crate: contents are computed from the rules (all must match).
    pub rules: Option<Vec<Rule>>,
}

impl Crate {
    pub fn new(name: &str) -> Self { Self { name: name.into(), ..Default::default() } }
    pub fn smart(name: &str, rules: Vec<Rule>) -> Self { Self { name: name.into(), rules: Some(rules), ..Default::default() } }
}

#[derive(Default)]
pub struct Library {
    tracks: Vec<TrackMeta>,
    pub crates: Vec<Crate>,
    history: Vec<TrackId>,
    prepare: Vec<TrackId>,
    next_id: TrackId,
}

impl Library {
    pub fn new() -> Self { Self::default() }

    pub fn add_track(&mut self, mut t: TrackMeta) -> TrackId {
        t.id = self.next_id;
        self.next_id += 1;
        self.tracks.push(t);
        self.next_id - 1
    }
    pub fn track(&self, id: TrackId) -> Option<&TrackMeta> { self.tracks.iter().find(|t| t.id == id) }
    fn track_mut(&mut self, id: TrackId) -> Option<&mut TrackMeta> { self.tracks.iter_mut().find(|t| t.id == id) }
    pub fn all(&self) -> &[TrackMeta] { &self.tracks }

    /// Every word must appear in title, artist or path (case-insensitive).
    pub fn search(&self, query: &str) -> Vec<&TrackMeta> {
        self.tracks.iter().filter(|t| text_match(t, query)).collect()
    }
    /// Search restricted to one crate's tracks.
    pub fn search_in(&self, c: &Crate, query: &str) -> Vec<&TrackMeta> {
        let ids = self.crate_tracks(c);
        self.tracks.iter().filter(|t| ids.contains(&t.id) && text_match(t, query)).collect()
    }
    pub fn crate_tracks(&self, c: &Crate) -> Vec<TrackId> {
        match &c.rules {
            Some(rules) => self.tracks.iter().filter(|t| rules.iter().all(|r| r.matches(t))).map(|t| t.id).collect(),
            None => c.tracks.clone(),
        }
    }
    pub fn set_rating(&mut self, id: TrackId, stars: u8) { if let Some(t) = self.track_mut(id) { t.rating = stars.min(5); } }
    pub fn set_color(&mut self, id: TrackId, color: Option<u32>) { if let Some(t) = self.track_mut(id) { t.color = color; } }
    pub fn set_analysis(&mut self, id: TrackId, bpm: Option<f32>, key: Option<String>) {
        if let Some(t) = self.track_mut(id) { t.bpm = bpm; t.key = key; }
    }

    /// Record that a track was played: bumps its play count and appends to history.
    pub fn mark_played(&mut self, id: TrackId) {
        if let Some(t) = self.track_mut(id) { t.plays += 1; self.history.push(id); }
    }
    pub fn history(&self) -> &[TrackId] { &self.history }

    pub fn prepare_add(&mut self, id: TrackId) { if !self.prepare.contains(&id) { self.prepare.push(id); } }
    pub fn prepare_remove(&mut self, id: TrackId) { self.prepare.retain(|&x| x != id); }
    pub fn prepare(&self) -> &[TrackId] { &self.prepare }

    /// Sort ids by BPM (tracks without a BPM go last).
    pub fn sorted_by_bpm(&self, ids: &[TrackId]) -> Vec<TrackId> {
        let mut v = ids.to_vec();
        v.sort_by(|a, b| {
            let (x, y) = (self.track(*a).and_then(|t| t.bpm), self.track(*b).and_then(|t| t.bpm));
            match (x, y) {
                (Some(x), Some(y)) => x.partial_cmp(&y).unwrap(),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            }
        });
        v
    }

    /// Update a track's path (relocate a missing file) and report whether it existed.
    pub fn relocate(&mut self, id: TrackId, new_path: &str) -> bool {
        match self.track_mut(id) { Some(t) => { t.path = new_path.into(); true } None => false }
    }
    /// Tracks whose file no longer exists on disk.
    pub fn missing_files(&self) -> Vec<TrackId> {
        self.tracks.iter().filter(|t| !std::path::Path::new(&t.path).exists()).map(|t| t.id).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lib() -> Library {
        let mut l = Library::new();
        for (title, artist, bpm, key, rating) in [
            ("Sunrise", "Alpha", 124.0, "A minor", 5u8),
            ("Night Drive", "Beta", 128.0, "C major", 3),
            ("Deep End", "Alpha", 90.0, "A minor", 1),
        ] {
            l.add_track(TrackMeta { path: format!("/music/{title}.mp3"), title: title.into(), artist: artist.into(),
                bpm: Some(bpm), key: Some(key.into()), rating, duration_s: 200.0, ..Default::default() });
        }
        l
    }

    #[test]
    fn search_matches_all_words_case_insensitively() {
        let l = lib();
        assert_eq!(l.search("alpha").len(), 2);
        assert_eq!(l.search("ALPHA deep").len(), 1);
        assert!(l.search("zzz").is_empty());
    }

    #[test]
    fn plain_and_smart_crates() {
        let mut l = lib();
        let mut c = Crate::new("Warmup");
        c.tracks = vec![0, 2];
        c.children.push(Crate::new("Sub"));
        assert_eq!(l.crate_tracks(&c), vec![0, 2]);
        let s = Crate::smart("House", vec![Rule::BpmBetween(120.0, 130.0), Rule::RatingAtLeast(4)]);
        assert_eq!(l.crate_tracks(&s), vec![0]);
        l.set_rating(1, 5);
        assert_eq!(l.crate_tracks(&s), vec![0, 1]);
        let k = Crate::smart("Am", vec![Rule::KeyIs("a minor".into())]);
        assert_eq!(l.crate_tracks(&k), vec![0, 2]);
        assert_eq!(l.search_in(&c, "sunrise").len(), 1);
    }

    #[test]
    fn plays_history_prepare() {
        let mut l = lib();
        l.mark_played(1);
        l.mark_played(1);
        l.mark_played(0);
        assert_eq!(l.track(1).unwrap().plays, 2);
        assert_eq!(l.history(), &[1, 1, 0]);
        l.prepare_add(2);
        l.prepare_add(2);
        assert_eq!(l.prepare(), &[2]);
        l.prepare_remove(2);
        assert!(l.prepare().is_empty());
        let plays = Crate::smart("Hot", vec![Rule::PlaysAtLeast(2)]);
        assert_eq!(l.crate_tracks(&plays), vec![1]);
    }

    #[test]
    fn sorting_relocating_missing() {
        let mut l = lib();
        assert_eq!(l.sorted_by_bpm(&[0, 1, 2]), vec![2, 0, 1]);
        l.set_analysis(2, None, None);
        assert_eq!(l.sorted_by_bpm(&[2, 1, 0]), vec![0, 1, 2]);
        assert_eq!(l.missing_files().len(), 3);
        assert!(l.relocate(0, "/etc/hostname") || l.relocate(0, "/"));
        assert_eq!(l.missing_files().len(), 2);
        l.set_rating(0, 9);
        assert_eq!(l.track(0).unwrap().rating, 5);
    }
}
