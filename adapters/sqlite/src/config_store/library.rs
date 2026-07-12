use rusqlite::Transaction;
use sona_core::config::{
    AppConfigLibrary, HotwordRuleRecord, HotwordSetRecord, PolishKeywordSetRecord,
    PolishPresetRecord, SpeakerProfileRecord, SpeakerProfileSampleRecord, SummaryTemplateRecord,
    TextReplacementRuleRecord, TextReplacementSetRecord,
};

use crate::DatabaseError;

const TEXT_REPLACEMENT: &str = "text_replacement";
const HOTWORD: &str = "hotword";
const POLISH_KEYWORD: &str = "polish_keyword";

pub(super) fn load(tx: &Transaction<'_>) -> Result<AppConfigLibrary, DatabaseError> {
    Ok(AppConfigLibrary {
        summary_templates: load_summary_templates(tx)?,
        polish_presets: load_polish_presets(tx)?,
        text_replacement_sets: load_text_replacement_sets(tx)?,
        hotword_sets: load_hotword_sets(tx)?,
        polish_keyword_sets: load_polish_keyword_sets(tx)?,
        speaker_profiles: load_speaker_profiles(tx)?,
    })
}

pub(super) fn replace(
    tx: &Transaction<'_>,
    library: &AppConfigLibrary,
    updated_at: i64,
) -> Result<(), DatabaseError> {
    tx.execute("DELETE FROM vocabulary_rules", [])?;
    tx.execute("DELETE FROM vocabulary_sets", [])?;
    tx.execute("DELETE FROM speaker_profile_samples", [])?;
    tx.execute("DELETE FROM speaker_profiles", [])?;
    tx.execute("DELETE FROM summary_templates", [])?;
    tx.execute("DELETE FROM polish_presets", [])?;

    save_summary_templates(tx, &library.summary_templates, updated_at)?;
    save_polish_presets(tx, &library.polish_presets, updated_at)?;
    save_text_replacement_sets(tx, &library.text_replacement_sets, updated_at)?;
    save_hotword_sets(tx, &library.hotword_sets, updated_at)?;
    save_polish_keyword_sets(tx, &library.polish_keyword_sets, updated_at)?;
    save_speaker_profiles(tx, &library.speaker_profiles, updated_at)
}

fn load_summary_templates(
    tx: &Transaction<'_>,
) -> Result<Vec<SummaryTemplateRecord>, DatabaseError> {
    let mut statement = tx.prepare_cached(
        "SELECT id, name, instructions
         FROM summary_templates
         ORDER BY sort_order, id",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(SummaryTemplateRecord {
            id: row.get(0)?,
            name: row.get(1)?,
            instructions: row.get(2)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(DatabaseError::QueryError)
}

fn save_summary_templates(
    tx: &Transaction<'_>,
    templates: &[SummaryTemplateRecord],
    updated_at: i64,
) -> Result<(), DatabaseError> {
    let mut statement = tx.prepare_cached(
        "INSERT INTO summary_templates (
            id, name, instructions, sort_order, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    for (sort_order, template) in templates.iter().enumerate() {
        statement.execute(rusqlite::params![
            template.id,
            template.name,
            template.instructions,
            sort_order as i64,
            updated_at,
            updated_at,
        ])?;
    }
    Ok(())
}

fn load_polish_presets(tx: &Transaction<'_>) -> Result<Vec<PolishPresetRecord>, DatabaseError> {
    let mut statement = tx.prepare_cached(
        "SELECT id, name, context
         FROM polish_presets
         ORDER BY sort_order, id",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(PolishPresetRecord {
            id: row.get(0)?,
            name: row.get(1)?,
            context: row.get(2)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(DatabaseError::QueryError)
}

fn save_polish_presets(
    tx: &Transaction<'_>,
    presets: &[PolishPresetRecord],
    updated_at: i64,
) -> Result<(), DatabaseError> {
    let mut statement = tx.prepare_cached(
        "INSERT INTO polish_presets (
            id, name, context, sort_order, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    for (sort_order, preset) in presets.iter().enumerate() {
        statement.execute(rusqlite::params![
            preset.id,
            preset.name,
            preset.context,
            sort_order as i64,
            updated_at,
            updated_at,
        ])?;
    }
    Ok(())
}

struct VocabularySetRow {
    id: String,
    name: String,
    enabled: bool,
    ignore_case: bool,
    keywords: String,
}

fn load_vocabulary_sets(
    tx: &Transaction<'_>,
    kind: &str,
) -> Result<Vec<VocabularySetRow>, DatabaseError> {
    let mut statement = tx.prepare_cached(
        "SELECT id, name, enabled, ignore_case, keywords
         FROM vocabulary_sets
         WHERE kind = ?1
         ORDER BY sort_order, id",
    )?;
    let rows = statement.query_map([kind], |row| {
        Ok(VocabularySetRow {
            id: row.get(0)?,
            name: row.get(1)?,
            enabled: row.get::<_, i64>(2)? != 0,
            ignore_case: row.get::<_, i64>(3)? != 0,
            keywords: row.get(4)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(DatabaseError::QueryError)
}

fn load_text_replacement_sets(
    tx: &Transaction<'_>,
) -> Result<Vec<TextReplacementSetRecord>, DatabaseError> {
    load_vocabulary_sets(tx, TEXT_REPLACEMENT)?
        .into_iter()
        .map(|row| {
            Ok(TextReplacementSetRecord {
                rules: load_text_replacement_rules(tx, &row.id)?,
                id: row.id,
                name: row.name,
                enabled: row.enabled,
                ignore_case: row.ignore_case,
            })
        })
        .collect()
}

fn load_hotword_sets(tx: &Transaction<'_>) -> Result<Vec<HotwordSetRecord>, DatabaseError> {
    load_vocabulary_sets(tx, HOTWORD)?
        .into_iter()
        .map(|row| {
            Ok(HotwordSetRecord {
                rules: load_hotword_rules(tx, &row.id)?,
                id: row.id,
                name: row.name,
                enabled: row.enabled,
            })
        })
        .collect()
}

fn load_polish_keyword_sets(
    tx: &Transaction<'_>,
) -> Result<Vec<PolishKeywordSetRecord>, DatabaseError> {
    Ok(load_vocabulary_sets(tx, POLISH_KEYWORD)?
        .into_iter()
        .map(|row| PolishKeywordSetRecord {
            id: row.id,
            name: row.name,
            enabled: row.enabled,
            keywords: row.keywords,
        })
        .collect())
}

fn insert_vocabulary_set(
    statement: &mut rusqlite::CachedStatement<'_>,
    id: &str,
    kind: &str,
    name: &str,
    enabled: bool,
    ignore_case: bool,
    keywords: &str,
    sort_order: usize,
    updated_at: i64,
) -> Result<(), DatabaseError> {
    statement.execute(rusqlite::params![
        id,
        kind,
        name,
        enabled as i64,
        ignore_case as i64,
        keywords,
        sort_order as i64,
        updated_at,
        updated_at,
    ])?;
    Ok(())
}

fn vocabulary_set_insert<'a>(
    tx: &'a Transaction<'_>,
) -> Result<rusqlite::CachedStatement<'a>, DatabaseError> {
    Ok(tx.prepare_cached(
        "INSERT INTO vocabulary_sets (
            id, kind, name, enabled, ignore_case, keywords, sort_order, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?)
}

fn save_text_replacement_sets(
    tx: &Transaction<'_>,
    sets: &[TextReplacementSetRecord],
    updated_at: i64,
) -> Result<(), DatabaseError> {
    let mut set_statement = vocabulary_set_insert(tx)?;
    let mut rule_statement = tx.prepare_cached(
        "INSERT INTO vocabulary_rules (
            id, set_kind, set_id, from_text, to_text, text, sort_order
         ) VALUES (?1, ?2, ?3, ?4, ?5, '', ?6)",
    )?;
    for (sort_order, set) in sets.iter().enumerate() {
        insert_vocabulary_set(
            &mut set_statement,
            &set.id,
            TEXT_REPLACEMENT,
            &set.name,
            set.enabled,
            set.ignore_case,
            "",
            sort_order,
            updated_at,
        )?;
        for (rule_order, rule) in set.rules.iter().enumerate() {
            rule_statement.execute(rusqlite::params![
                rule.id,
                TEXT_REPLACEMENT,
                set.id,
                rule.from,
                rule.to,
                rule_order as i64,
            ])?;
        }
    }
    Ok(())
}

fn save_hotword_sets(
    tx: &Transaction<'_>,
    sets: &[HotwordSetRecord],
    updated_at: i64,
) -> Result<(), DatabaseError> {
    let mut set_statement = vocabulary_set_insert(tx)?;
    let mut rule_statement = tx.prepare_cached(
        "INSERT INTO vocabulary_rules (
            id, set_kind, set_id, from_text, to_text, text, sort_order
         ) VALUES (?1, ?2, ?3, '', '', ?4, ?5)",
    )?;
    for (sort_order, set) in sets.iter().enumerate() {
        insert_vocabulary_set(
            &mut set_statement,
            &set.id,
            HOTWORD,
            &set.name,
            set.enabled,
            false,
            "",
            sort_order,
            updated_at,
        )?;
        for (rule_order, rule) in set.rules.iter().enumerate() {
            rule_statement.execute(rusqlite::params![
                rule.id,
                HOTWORD,
                set.id,
                rule.text,
                rule_order as i64,
            ])?;
        }
    }
    Ok(())
}

fn save_polish_keyword_sets(
    tx: &Transaction<'_>,
    sets: &[PolishKeywordSetRecord],
    updated_at: i64,
) -> Result<(), DatabaseError> {
    let mut statement = vocabulary_set_insert(tx)?;
    for (sort_order, set) in sets.iter().enumerate() {
        insert_vocabulary_set(
            &mut statement,
            &set.id,
            POLISH_KEYWORD,
            &set.name,
            set.enabled,
            false,
            &set.keywords,
            sort_order,
            updated_at,
        )?;
    }
    Ok(())
}

fn load_text_replacement_rules(
    tx: &Transaction<'_>,
    set_id: &str,
) -> Result<Vec<TextReplacementRuleRecord>, DatabaseError> {
    let mut statement = tx.prepare_cached(
        "SELECT id, from_text, to_text
         FROM vocabulary_rules
         WHERE set_kind = ?1 AND set_id = ?2
         ORDER BY sort_order, id",
    )?;
    let rows = statement.query_map(rusqlite::params![TEXT_REPLACEMENT, set_id], |row| {
        Ok(TextReplacementRuleRecord {
            id: row.get(0)?,
            from: row.get(1)?,
            to: row.get(2)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(DatabaseError::QueryError)
}

fn load_hotword_rules(
    tx: &Transaction<'_>,
    set_id: &str,
) -> Result<Vec<HotwordRuleRecord>, DatabaseError> {
    let mut statement = tx.prepare_cached(
        "SELECT id, text
         FROM vocabulary_rules
         WHERE set_kind = ?1 AND set_id = ?2
         ORDER BY sort_order, id",
    )?;
    let rows = statement.query_map(rusqlite::params![HOTWORD, set_id], |row| {
        Ok(HotwordRuleRecord {
            id: row.get(0)?,
            text: row.get(1)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(DatabaseError::QueryError)
}

fn load_speaker_profiles(tx: &Transaction<'_>) -> Result<Vec<SpeakerProfileRecord>, DatabaseError> {
    let mut statement = tx.prepare_cached(
        "SELECT id, name, enabled
         FROM speaker_profiles
         ORDER BY sort_order, id",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)? != 0,
        ))
    })?;
    let rows = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(DatabaseError::QueryError)?;
    rows.into_iter()
        .map(|(id, name, enabled)| {
            Ok(SpeakerProfileRecord {
                samples: load_speaker_profile_samples(tx, &id)?,
                id,
                name,
                enabled,
            })
        })
        .collect()
}

fn load_speaker_profile_samples(
    tx: &Transaction<'_>,
    profile_id: &str,
) -> Result<Vec<SpeakerProfileSampleRecord>, DatabaseError> {
    let mut statement = tx.prepare_cached(
        "SELECT id, file_path, source_name, duration_seconds
         FROM speaker_profile_samples
         WHERE profile_id = ?1
         ORDER BY sort_order, id",
    )?;
    let rows = statement.query_map([profile_id], |row| {
        Ok(SpeakerProfileSampleRecord {
            id: row.get(0)?,
            file_path: row.get(1)?,
            source_name: row.get(2)?,
            duration_seconds: row.get(3)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(DatabaseError::QueryError)
}

fn save_speaker_profiles(
    tx: &Transaction<'_>,
    profiles: &[SpeakerProfileRecord],
    updated_at: i64,
) -> Result<(), DatabaseError> {
    let mut profile_statement = tx.prepare_cached(
        "INSERT INTO speaker_profiles (
            id, name, enabled, sort_order, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    let mut sample_statement = tx.prepare_cached(
        "INSERT INTO speaker_profile_samples (
            id, profile_id, file_path, source_name, duration_seconds, sort_order
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    for (sort_order, profile) in profiles.iter().enumerate() {
        profile_statement.execute(rusqlite::params![
            profile.id,
            profile.name,
            profile.enabled as i64,
            sort_order as i64,
            updated_at,
            updated_at,
        ])?;
        for (sample_order, sample) in profile.samples.iter().enumerate() {
            sample_statement.execute(rusqlite::params![
                sample.id,
                profile.id,
                sample.file_path,
                sample.source_name,
                sample.duration_seconds,
                sample_order as i64,
            ])?;
        }
    }
    Ok(())
}
