//! Learning from the user's own corrections. When a History entry is edited
//! (or the original is restored after refinement changed a word), the word-level
//! difference is counted in a small local store; a substitution seen twice
//! becomes a suggestion in Settings > Vocabulary. Accepting one adds the target
//! term to the vocabulary and, for plain edits, a deterministic replacement
//! rule applied to every later transcript (see [`apply_replacements`]).
//!
//! Only the changed words are stored, never the surrounding text, and nothing
//! here logs text.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

use crate::config;

/// How many times the same substitution must be seen before it is suggested.
pub const SUGGEST_AFTER: u32 = 2;
/// Longest phrase on either side of a substitution, in words.
const MAX_PHRASE_WORDS: usize = 3;
/// A change touching more separate places than this is a rewrite, not a fix.
const MAX_HUNKS: usize = 3;
/// Past this many words the diff is skipped (it is quadratic, and a text this
/// long being edited is not a single-word fix anyway).
const MAX_DIFF_WORDS: usize = 400;
/// Bounds the store file; one-off substitutions are dropped first.
const MAX_STORED: usize = 500;

/// Words that are too common to turn into a global rewrite rule from a single
/// edit ("their" -> "there" fixes one sentence, not every later one).
const COMMON_WORDS: &[&str] = &[
    "a", "an", "the", "to", "too", "two", "of", "in", "on", "at", "it", "its", "is", "are", "was",
    "were", "be", "and", "or", "but", "so", "i", "you", "we", "he", "she", "they", "them", "this",
    "that", "there", "their", "here", "hear", "for", "four", "with", "no", "know", "now", "new",
    "not", "by", "buy", "my", "me", "do", "can", "will", "would", "your", "you're",
];

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// The user edited an entry's text: `from` is what was delivered.
    #[default]
    Edit,
    /// The user restored the pre-refinement text: `from` is what the refiner
    /// wrote, `to` what it was before. Never becomes a replacement rule (it
    /// would rewrite correct words); only the vocabulary term is added so the
    /// refiner is told to leave it alone.
    Original,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum State {
    #[default]
    Pending,
    Accepted,
    Dismissed,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Correction {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub source: Source,
    pub count: u32,
    #[serde(default)]
    pub state: State,
}

/// A `from` -> `to` rewrite applied to transcripts before formatting.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Replacement {
    pub from: String,
    pub to: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct CorrectionStore {
    #[serde(default)]
    pub corrections: Vec<Correction>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Substitution {
    pub from: String,
    pub to: String,
}

struct Word {
    /// Lowercased with surrounding punctuation stripped; what is compared.
    norm: String,
    /// Same, but keeping case; what is stored as the target.
    shown: String,
}

fn words(text: &str) -> Vec<Word> {
    text.split_whitespace()
        .filter_map(|token| {
            let shown = token.trim_matches(|c: char| !c.is_alphanumeric());
            (!shown.is_empty() && shown.chars().count() <= 40).then(|| Word {
                norm: shown.to_lowercase(),
                shown: shown.to_string(),
            })
        })
        .collect()
}

/// Single- or short-phrase substitutions that turn `old` into `new`, compared
/// word by word. Casing and punctuation changes are ignored, as are pure
/// insertions and deletions, long phrases, and edits that rewrite most of the
/// text.
pub fn extract_substitutions(old: &str, new: &str) -> Vec<Substitution> {
    let (a, b) = (words(old), words(new));
    let (n, m) = (a.len(), b.len());
    if n == 0 || m == 0 || n > MAX_DIFF_WORDS || m > MAX_DIFF_WORDS {
        return Vec::new();
    }

    // lcs[i][j]: longest common run of words in a[i..] and b[j..].
    let mut lcs = vec![vec![0u16; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i].norm == b[j].norm {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let common = lcs[0][0] as usize;
    if common * 2 < n.max(m) {
        return Vec::new();
    }

    // Walk the table, collecting each maximal stretch of non-matching words.
    let mut hunks: Vec<(Vec<&Word>, Vec<&Word>)> = Vec::new();
    let (mut i, mut j) = (0, 0);
    let (mut removed, mut added): (Vec<&Word>, Vec<&Word>) = (Vec::new(), Vec::new());
    while i < n || j < m {
        if i < n && j < m && a[i].norm == b[j].norm {
            if !removed.is_empty() || !added.is_empty() {
                hunks.push((std::mem::take(&mut removed), std::mem::take(&mut added)));
            }
            i += 1;
            j += 1;
        } else if j >= m || (i < n && lcs[i + 1][j] >= lcs[i][j + 1]) {
            removed.push(&a[i]);
            i += 1;
        } else {
            added.push(&b[j]);
            j += 1;
        }
    }
    if !removed.is_empty() || !added.is_empty() {
        hunks.push((removed, added));
    }
    if hunks.len() > MAX_HUNKS {
        return Vec::new();
    }

    hunks
        .into_iter()
        .filter(|(from, to)| {
            (1..=MAX_PHRASE_WORDS).contains(&from.len())
                && (1..=MAX_PHRASE_WORDS).contains(&to.len())
        })
        .map(|(from, to)| Substitution {
            from: from
                .iter()
                .map(|w| w.norm.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            to: to
                .iter()
                .map(|w| w.shown.as_str())
                .collect::<Vec<_>>()
                .join(" "),
        })
        .collect()
}

impl CorrectionStore {
    /// Counts each substitution; returns true if any suggestion just became
    /// visible (reached the threshold).
    pub fn record(&mut self, source: Source, subs: Vec<Substitution>) -> bool {
        let mut newly_suggested = false;
        for sub in subs {
            // A rule from a common word would rewrite ordinary speech.
            if source == Source::Edit
                && !sub.from.contains(' ')
                && COMMON_WORDS.contains(&sub.from.as_str())
            {
                continue;
            }
            match self
                .corrections
                .iter_mut()
                .find(|c| c.source == source && c.from == sub.from && c.to == sub.to)
            {
                Some(existing) => {
                    existing.count += 1;
                    newly_suggested |=
                        existing.state == State::Pending && existing.count == SUGGEST_AFTER;
                }
                None => self.corrections.push(Correction {
                    from: sub.from,
                    to: sub.to,
                    source,
                    count: 1,
                    state: State::Pending,
                }),
            }
        }
        while self.corrections.len() > MAX_STORED {
            match self
                .corrections
                .iter()
                .position(|c| c.count < SUGGEST_AFTER)
            {
                Some(oldest) => self.corrections.remove(oldest),
                None => self.corrections.remove(0),
            };
        }
        newly_suggested
    }

    pub fn suggestions(&self) -> Vec<Correction> {
        self.corrections
            .iter()
            .filter(|c| c.state == State::Pending && c.count >= SUGGEST_AFTER)
            .cloned()
            .collect()
    }

    fn resolve(&mut self, from: &str, to: &str, source: Source, state: State) -> bool {
        match self
            .corrections
            .iter_mut()
            .find(|c| c.source == source && c.from == from && c.to == to)
        {
            Some(c) => {
                c.state = state;
                true
            }
            None => false,
        }
    }
}

fn store_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("corrections.json"))
}

/// A missing or unreadable file is an empty store: this is only a hint, so
/// losing it is cheap.
fn load_from(path: &Path) -> CorrectionStore {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_to(path: &Path, store: &CorrectionStore) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string(store).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// Serializes read-modify-write of the store file.
static STORE_LOCK: Mutex<()> = Mutex::new(());

/// Records what changed between `old` and `new` if the user hasn't turned
/// learning off. Never fails the edit it piggybacks on.
pub fn learn(app: &AppHandle, old: &str, new: &str, source: Source) {
    if !config::load(app).learn_corrections {
        return;
    }
    let subs = extract_substitutions(old, new);
    if subs.is_empty() {
        return;
    }
    let Ok(path) = store_path(app) else { return };
    let _guard = STORE_LOCK.lock().unwrap();
    let mut store = load_from(&path);
    store.record(source, subs);
    if save_to(&path, &store).is_ok() {
        let _ = app.emit("corrections-updated", ());
    }
}

fn update_store(
    app: &AppHandle,
    f: impl FnOnce(&mut CorrectionStore) -> bool,
) -> Result<(), String> {
    let path = store_path(app)?;
    let _guard = STORE_LOCK.lock().unwrap();
    let mut store = load_from(&path);
    if f(&mut store) {
        save_to(&path, &store)?;
        let _ = app.emit("corrections-updated", ());
    }
    Ok(())
}

/// Whether `word` (a vocabulary term) is already in the list, ignoring case.
fn has_term(terms: &[String], word: &str) -> bool {
    terms.iter().any(|t| t.eq_ignore_ascii_case(word))
}

/// Adds `to` to the vocabulary and, for an edit, a replacement rule from
/// `from`. Pure so the accept path is testable without an app.
fn accept_into(cfg: &mut config::AppConfig, c: &Correction) {
    if !has_term(&cfg.vocabulary, &c.to) {
        cfg.vocabulary.push(c.to.clone());
    }
    if c.source == Source::Edit
        && !cfg
            .replacements
            .iter()
            .any(|r| r.from.eq_ignore_ascii_case(&c.from))
    {
        cfg.replacements.push(Replacement {
            from: c.from.clone(),
            to: c.to.clone(),
        });
    }
}

#[tauri::command]
pub fn get_learn_corrections(app: AppHandle) -> bool {
    config::load(&app).learn_corrections
}

#[tauri::command]
pub fn set_learn_corrections(app: AppHandle, enabled: bool) {
    let mut cfg = config::load(&app);
    cfg.learn_corrections = enabled;
    let _ = config::save(&app, &cfg);
}

// Off the main thread: sync commands run on it, and this one does I/O.
#[tauri::command(async)]
pub fn get_correction_suggestions(app: AppHandle) -> Vec<Correction> {
    store_path(&app)
        .map(|p| load_from(&p).suggestions())
        .unwrap_or_default()
}

#[tauri::command(async)]
pub fn accept_correction_suggestion(
    app: AppHandle,
    from: String,
    to: String,
    source: Source,
    state: tauri::State<crate::recording::RecordingState>,
) -> Result<(), String> {
    let correction = Correction {
        from: from.clone(),
        to: to.clone(),
        source,
        count: 0,
        state: State::Accepted,
    };
    let mut cfg = config::load(&app);
    accept_into(&mut cfg, &correction);
    state.whisper.set_vocabulary(cfg.vocabulary.clone());
    config::save(&app, &cfg)?;
    update_store(&app, |s| s.resolve(&from, &to, source, State::Accepted))
}

#[tauri::command(async)]
pub fn dismiss_correction_suggestion(
    app: AppHandle,
    from: String,
    to: String,
    source: Source,
) -> Result<(), String> {
    update_store(&app, |s| s.resolve(&from, &to, source, State::Dismissed))
}

#[tauri::command]
pub fn get_replacements(app: AppHandle) -> Vec<Replacement> {
    config::load(&app).replacements
}

#[tauri::command]
pub fn set_replacements(app: AppHandle, replacements: Vec<Replacement>) {
    let mut cfg = config::load(&app);
    cfg.replacements = replacements;
    let _ = config::save(&app, &cfg);
}

fn same_char(a: char, b: char) -> bool {
    a == b || a.to_lowercase().eq(b.to_lowercase())
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Length in chars of the match of `phrase` (words separated by any run of
/// whitespace) at the start of `at`, or `None`.
fn match_phrase(at: &[char], phrase: &[Vec<char>]) -> Option<usize> {
    let mut pos = 0;
    for (k, word) in phrase.iter().enumerate() {
        if k > 0 {
            let start = pos;
            while at.get(pos).is_some_and(|c| c.is_whitespace()) {
                pos += 1;
            }
            if pos == start {
                return None;
            }
        }
        for &c in word {
            if !at.get(pos).is_some_and(|&t| same_char(t, c)) {
                return None;
            }
            pos += 1;
        }
    }
    // Whole words only: "kama" must not match inside "kamala".
    (!at.get(pos).is_some_and(|&c| is_word_char(c))).then_some(pos)
}

/// Matches the capitalization of what was heard: a capitalized or
/// all-caps match capitalizes a lowercase target, but a target that already
/// has capitals ("Postgres") is used as written.
fn cased(matched: &str, target: &str) -> String {
    if target.chars().any(char::is_uppercase) {
        return target.to_string();
    }
    let letters: Vec<char> = matched.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.len() > 1 && letters.iter().all(|c| c.is_uppercase()) {
        return target.to_uppercase();
    }
    if letters.first().is_some_and(|c| c.is_uppercase()) {
        let mut chars = target.chars();
        return match chars.next() {
            Some(first) => first.to_uppercase().chain(chars).collect(),
            None => String::new(),
        };
    }
    target.to_string()
}

/// Applies the user's replacement rules to a transcript: whole-word,
/// case-insensitive, tolerant of extra whitespace inside a phrase.
pub fn apply_replacements(text: &str, rules: &[Replacement]) -> String {
    let mut text = text.to_string();
    for rule in rules {
        let phrase: Vec<Vec<char>> = rule
            .from
            .split_whitespace()
            .map(|w| w.chars().collect())
            .collect();
        if phrase.is_empty() || rule.to.trim().is_empty() {
            continue;
        }
        let chars: Vec<char> = text.chars().collect();
        let mut out = String::with_capacity(text.len());
        let mut i = 0;
        while i < chars.len() {
            let at_boundary = i == 0 || !is_word_char(chars[i - 1]);
            if at_boundary {
                if let Some(len) = match_phrase(&chars[i..], &phrase) {
                    let matched: String = chars[i..i + len].iter().collect();
                    out.push_str(&cased(&matched, rule.to.trim()));
                    i += len;
                    continue;
                }
            }
            out.push(chars[i]);
            i += 1;
        }
        text = out;
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sub(from: &str, to: &str) -> Substitution {
        Substitution {
            from: from.into(),
            to: to.into(),
        }
    }

    #[test]
    fn extracts_a_single_word_substitution() {
        assert_eq!(
            extract_substitutions("Add a kama here and there.", "Add a comma here and there."),
            vec![sub("kama", "comma")]
        );
    }

    #[test]
    fn extracts_a_phrase_substitution_keeping_target_case() {
        assert_eq!(
            extract_substitutions(
                "We use post gress for storage.",
                "We use Postgres for storage."
            ),
            vec![sub("post gress", "Postgres")]
        );
    }

    #[test]
    fn ignores_casing_and_punctuation_only_changes() {
        assert!(extract_substitutions("hello world", "Hello, world!").is_empty());
        assert!(extract_substitutions("ship it now", "Ship it. Now").is_empty());
    }

    #[test]
    fn ignores_pure_insertions_and_deletions() {
        assert!(extract_substitutions("ship it", "ship it now please").is_empty());
        assert!(extract_substitutions("ship it now please", "ship it").is_empty());
    }

    #[test]
    fn ignores_large_rewrites() {
        assert!(extract_substitutions(
            "the quick brown fox jumps over the lazy dog",
            "a slow green turtle crawls under some tired cat",
        )
        .is_empty());
        // Four separate edits in one text is a rewrite, not a fix.
        assert!(extract_substitutions(
            "one aa two bb three cc four dd five",
            "one a2 two b2 three c2 four d2 five",
        )
        .is_empty());
    }

    #[test]
    fn ignores_long_phrase_swaps() {
        assert!(extract_substitutions(
            "keep this part and then some extra words end",
            "keep this part and a b c d e end",
        )
        .is_empty());
    }

    #[test]
    fn finds_several_independent_fixes() {
        let subs = extract_substitutions(
            "the kama and the post gress url are fine today ok",
            "the comma and the Postgres url are fine today ok",
        );
        assert_eq!(
            subs,
            vec![sub("kama", "comma"), sub("post gress", "Postgres")]
        );
    }

    #[test]
    fn empty_sides_produce_nothing() {
        assert!(extract_substitutions("", "hello").is_empty());
        assert!(extract_substitutions("hello", "...").is_empty());
    }

    #[test]
    fn suggests_only_after_the_threshold() {
        let mut store = CorrectionStore::default();
        assert!(!store.record(Source::Edit, vec![sub("kama", "comma")]));
        assert!(store.suggestions().is_empty());
        assert!(store.record(Source::Edit, vec![sub("kama", "comma")]));
        let suggestions = store.suggestions();
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].count, 2);
        // A different target is a different substitution.
        store.record(Source::Edit, vec![sub("kama", "Kama")]);
        assert_eq!(store.suggestions().len(), 1);
    }

    #[test]
    fn dismissed_and_accepted_suggestions_stay_gone() {
        let mut store = CorrectionStore::default();
        for _ in 0..3 {
            store.record(Source::Edit, vec![sub("kama", "comma")]);
        }
        assert!(store.resolve("kama", "comma", Source::Edit, State::Dismissed));
        store.record(Source::Edit, vec![sub("kama", "comma")]);
        assert!(store.suggestions().is_empty());
        assert!(!store.resolve("nope", "x", Source::Edit, State::Dismissed));
    }

    #[test]
    fn sources_are_counted_separately() {
        let mut store = CorrectionStore::default();
        store.record(Source::Edit, vec![sub("a1", "b1")]);
        store.record(Source::Original, vec![sub("a1", "b1")]);
        assert!(store.suggestions().is_empty());
    }

    #[test]
    fn common_words_never_become_edit_suggestions() {
        let mut store = CorrectionStore::default();
        for _ in 0..3 {
            store.record(Source::Edit, vec![sub("their", "there")]);
        }
        assert!(store.suggestions().is_empty());
    }

    #[test]
    fn store_is_bounded_and_drops_one_offs_first() {
        let mut store = CorrectionStore::default();
        for _ in 0..2 {
            store.record(Source::Edit, vec![sub("keeper", "Keeper")]);
        }
        for i in 0..MAX_STORED + 20 {
            store.record(Source::Edit, vec![sub(&format!("w{i}"), "x")]);
        }
        assert_eq!(store.corrections.len(), MAX_STORED);
        assert_eq!(store.suggestions().len(), 1);
    }

    #[test]
    fn store_round_trips_through_a_file() {
        let dir = std::env::temp_dir().join(format!("dw-corrections-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("corrections.json");
        assert_eq!(load_from(&path), CorrectionStore::default());

        let mut store = CorrectionStore::default();
        store.record(Source::Edit, vec![sub("kama", "comma")]);
        store.record(Source::Original, vec![sub("kubectl", "cube control")]);
        store.resolve("kama", "comma", Source::Edit, State::Dismissed);
        save_to(&path, &store).unwrap();
        assert_eq!(load_from(&path), store);

        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(load_from(&path), CorrectionStore::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn accepting_adds_vocabulary_and_a_rule_once() {
        let mut cfg = config::AppConfig {
            vocabulary: vec!["Tauri".into()],
            ..config::AppConfig::default()
        };
        let edit = Correction {
            from: "kama".into(),
            to: "comma".into(),
            source: Source::Edit,
            count: 2,
            state: State::Pending,
        };
        accept_into(&mut cfg, &edit);
        accept_into(&mut cfg, &edit);
        assert_eq!(
            cfg.vocabulary,
            vec!["Tauri".to_string(), "comma".to_string()]
        );
        assert_eq!(
            cfg.replacements,
            vec![Replacement {
                from: "kama".into(),
                to: "comma".into()
            }]
        );
    }

    #[test]
    fn accepting_a_restored_original_adds_no_rule() {
        let mut cfg = config::AppConfig::default();
        cfg.vocabulary.clear();
        accept_into(
            &mut cfg,
            &Correction {
                from: "cube control".into(),
                to: "kubectl".into(),
                source: Source::Original,
                count: 2,
                state: State::Pending,
            },
        );
        assert_eq!(cfg.vocabulary, vec!["kubectl".to_string()]);
        assert!(cfg.replacements.is_empty());
    }

    fn rule(from: &str, to: &str) -> Replacement {
        Replacement {
            from: from.into(),
            to: to.into(),
        }
    }

    #[test]
    fn replacement_matches_whole_words_only() {
        let rules = [rule("kama", "comma")];
        assert_eq!(
            apply_replacements("add a kama here", &rules),
            "add a comma here"
        );
        assert_eq!(
            apply_replacements("kamala and kama2", &rules),
            "kamala and kama2"
        );
        assert_eq!(
            apply_replacements("a kama, then kama.", &rules),
            "a comma, then comma."
        );
    }

    #[test]
    fn replacement_is_case_aware() {
        let rules = [rule("kama", "comma"), rule("post gress", "Postgres")];
        assert_eq!(apply_replacements("Kama first", &rules), "Comma first");
        assert_eq!(apply_replacements("KAMA", &rules), "COMMA");
        assert_eq!(
            apply_replacements("use Post Gress now", &rules),
            "use Postgres now"
        );
        assert_eq!(
            apply_replacements("use post   gress now", &rules),
            "use Postgres now"
        );
    }

    #[test]
    fn replacement_ignores_empty_rules_and_partial_phrases() {
        let rules = [
            rule("", "x"),
            rule("post gress", ""),
            rule("post gress", "Postgres"),
        ];
        assert_eq!(apply_replacements("post office", &rules), "post office");
        assert_eq!(apply_replacements("", &rules), "");
    }
}
