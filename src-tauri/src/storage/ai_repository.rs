use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::models::ai::{
    AiAnalysisRecord, AnalysisResultStatus, Category, CategoryTemplate, TemplateCategory,
};

pub fn upsert_category_template(
    connection: &mut Connection,
    template_id: &str,
    name: &str,
    categories: &[TemplateCategory],
    now: &str,
) -> rusqlite::Result<CategoryTemplate> {
    let transaction = connection.transaction()?;
    let previous_version: Option<i64> = transaction
        .query_row(
            "SELECT version FROM ai_category_templates WHERE template_id = ?1",
            [template_id],
            |row| row.get(0),
        )
        .optional()?;
    let version = previous_version.unwrap_or(0) + 1;
    transaction.execute(
        "INSERT INTO ai_category_templates(template_id, name, version, created_at, updated_at)
         VALUES (?1, ?2, ?3, COALESCE((SELECT created_at FROM ai_category_templates WHERE template_id = ?1), ?4), ?4)
         ON CONFLICT(template_id) DO UPDATE SET name = excluded.name, version = excluded.version, updated_at = excluded.updated_at",
        params![template_id, name, version, now],
    )?;
    transaction.execute(
        "DELETE FROM ai_category_template_items WHERE template_id = ?1",
        [template_id],
    )?;
    {
        let mut statement = transaction.prepare(
            "INSERT INTO ai_category_template_items(template_id, category_id, name, description, default_enabled)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for category in categories {
            statement.execute(params![
                template_id,
                category.id,
                category.name,
                category.description,
                category.default_enabled,
            ])?;
        }
    }
    transaction.commit()?;
    Ok(CategoryTemplate {
        id: template_id.into(),
        name: name.into(),
        version,
        categories: categories.to_vec(),
    })
}

pub fn read_category_templates(connection: &Connection) -> rusqlite::Result<Vec<CategoryTemplate>> {
    let mut statement = connection.prepare(
        "SELECT template_id, name, version FROM ai_category_templates ORDER BY template_id",
    )?;
    let headers: Vec<(String, String, i64)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<Result<_, _>>()?;
    headers
        .into_iter()
        .map(|(id, name, version)| {
            Ok(CategoryTemplate {
                categories: read_template_categories(connection, &id)?,
                id,
                name,
                version,
            })
        })
        .collect()
}

pub fn read_category_template(
    connection: &Connection,
    template_id: &str,
) -> rusqlite::Result<Option<CategoryTemplate>> {
    let header = connection
        .query_row(
            "SELECT template_id, name, version FROM ai_category_templates WHERE template_id = ?1",
            [template_id],
            |row| {
                let id: String = row.get(0)?;
                let name: String = row.get(1)?;
                let version: i64 = row.get(2)?;
                Ok((id, name, version))
            },
        )
        .optional()?;
    header
        .map(|(id, name, version)| {
            Ok(CategoryTemplate {
                categories: read_template_categories(connection, &id)?,
                id,
                name,
                version,
            })
        })
        .transpose()
}

pub fn delete_category_template(
    connection: &Connection,
    template_id: &str,
) -> rusqlite::Result<bool> {
    connection.execute(
        "DELETE FROM ai_root_category_templates WHERE template_id = ?1",
        [template_id],
    )?;
    let deleted = connection.execute(
        "DELETE FROM ai_category_templates WHERE template_id = ?1",
        [template_id],
    )? == 1;
    Ok(deleted)
}

pub fn bind_root_to_category_template(
    connection: &Connection,
    root_path: &str,
    template_id: &str,
    template_version: i64,
) -> rusqlite::Result<()> {
    connection.execute(
        "INSERT INTO ai_root_category_templates(root_path, template_id, template_version)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(root_path) DO UPDATE SET template_id = excluded.template_id, template_version = excluded.template_version",
        params![root_path, template_id, template_version],
    )?;
    Ok(())
}

pub fn read_root_category_template(
    connection: &Connection,
    root_path: &str,
) -> rusqlite::Result<Option<(String, i64)>> {
    connection
        .query_row(
            "SELECT template_id, template_version FROM ai_root_category_templates WHERE root_path = ?1",
            [root_path],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
}

pub fn read_template_categories(
    connection: &Connection,
    template_id: &str,
) -> rusqlite::Result<Vec<TemplateCategory>> {
    let mut statement = connection.prepare(
        "SELECT category_id, name, description, default_enabled
         FROM ai_category_template_items WHERE template_id = ?1 ORDER BY category_id",
    )?;
    statement
        .query_map([template_id], |row| {
            Ok(TemplateCategory {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                default_enabled: row.get(3)?,
            })
        })?
        .collect()
}

pub fn replace_categories(
    connection: &mut Connection,
    root_path: &str,
    categories: &[Category],
) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute(
        "DELETE FROM ai_categories WHERE root_path = ?1",
        [root_path],
    )?;
    {
        let mut statement = transaction.prepare(
            "INSERT INTO ai_categories (root_path, category_id, name, description, directory_path, enabled)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        for category in categories {
            statement.execute(params![
                root_path,
                category.id,
                category.name,
                category.description,
                category.directory_path,
                category.enabled,
            ])?;
        }
    }
    transaction.commit()
}

pub fn migrate_category_directories(connection: &Connection) -> rusqlite::Result<()> {
    let mut statement = connection.prepare(
        "SELECT root_path, category_id FROM ai_categories ORDER BY root_path, category_id",
    )?;
    let rows: Vec<(String, String)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    for (root_path, category_id) in rows {
        let directory = Path::new(&root_path).join(&category_id);
        connection.execute(
            "UPDATE ai_categories SET directory_path = ?3
             WHERE root_path = ?1 AND category_id = ?2",
            params![
                root_path,
                category_id,
                directory.to_string_lossy().to_string()
            ],
        )?;
    }
    Ok(())
}

pub fn read_categories(
    connection: &Connection,
    root_path: &str,
) -> rusqlite::Result<Vec<Category>> {
    let mut statement = connection.prepare(
        "SELECT category_id, name, description, directory_path, enabled
         FROM ai_categories WHERE root_path = ?1 ORDER BY category_id",
    )?;
    statement
        .query_map([root_path], |row| {
            Ok(Category {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                directory_path: row.get(3)?,
                enabled: row.get(4)?,
            })
        })?
        .collect()
}

pub fn delete_category(
    connection: &Connection,
    root_path: &str,
    category_id: &str,
) -> rusqlite::Result<bool> {
    Ok(connection.execute(
        "DELETE FROM ai_categories WHERE root_path = ?1 AND category_id = ?2",
        params![root_path, category_id],
    )? == 1)
}

pub fn count_pending_category_references(
    connection: &Connection,
    root_path: &str,
    category_id: &str,
) -> rusqlite::Result<i64> {
    connection.query_row(
        "SELECT COUNT(*) FROM ai_analysis_results
         WHERE root_path = ?1 AND category_id = ?2 AND status = 'pending'",
        params![root_path, category_id],
        |row| row.get(0),
    )
}

pub fn insert_analysis_result(
    connection: &Connection,
    record: &AiAnalysisRecord,
) -> rusqlite::Result<()> {
    connection.execute(
        "INSERT INTO ai_analysis_results (
            id, batch_id, root_path, source_path, content_fingerprint, provider, model,
            prompt_version, template_id, template_version, summary, keywords_json,
            suggested_filename, category_id, confidence, reason, status, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
        params![
            record.id,
            record.batch_id,
            record.root_path,
            record.source_path,
            record.content_fingerprint,
            record.provider,
            record.model,
            record.prompt_version,
            record.template_id,
            record.template_version,
            record.summary,
            serde_json::to_string(&record.keywords)
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?,
            record.suggested_filename,
            record.category_id,
            record.confidence,
            record.reason,
            record.status.as_str(),
            record.created_at,
        ],
    )?;
    Ok(())
}

pub fn read_batch_results(
    connection: &Connection,
    batch_id: &str,
) -> rusqlite::Result<Vec<AiAnalysisRecord>> {
    let mut statement = connection.prepare(
        "SELECT id, batch_id, root_path, source_path, content_fingerprint, provider, model,
                prompt_version, template_id, template_version, summary, keywords_json,
                suggested_filename, category_id, confidence, reason, status, created_at
         FROM ai_analysis_results WHERE batch_id = ?1 ORDER BY created_at, id",
    )?;
    statement.query_map([batch_id], map_record)?.collect()
}

pub fn delete_batch_results(connection: &Connection, batch_id: &str) -> rusqlite::Result<()> {
    connection.execute(
        "DELETE FROM ai_analysis_results WHERE batch_id = ?1",
        [batch_id],
    )?;
    Ok(())
}

pub fn read_result(
    connection: &Connection,
    result_id: &str,
) -> rusqlite::Result<Option<AiAnalysisRecord>> {
    connection
        .query_row(
            "SELECT id, batch_id, root_path, source_path, content_fingerprint, provider, model,
                    prompt_version, template_id, template_version, summary, keywords_json,
                    suggested_filename, category_id, confidence, reason, status, created_at
             FROM ai_analysis_results WHERE id = ?1",
            [result_id],
            map_record,
        )
        .optional()
}

pub fn update_result_status(
    connection: &Connection,
    result_id: &str,
    status: AnalysisResultStatus,
) -> rusqlite::Result<bool> {
    Ok(connection.execute(
        "UPDATE ai_analysis_results SET status = ?2 WHERE id = ?1",
        params![result_id, status.as_str()],
    )? == 1)
}

fn map_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<AiAnalysisRecord> {
    let keywords_json: String = row.get(11)?;
    let status: String = row.get(16)?;
    Ok(AiAnalysisRecord {
        id: row.get(0)?,
        batch_id: row.get(1)?,
        root_path: row.get(2)?,
        source_path: row.get(3)?,
        content_fingerprint: row.get(4)?,
        provider: row.get(5)?,
        model: row.get(6)?,
        prompt_version: row.get(7)?,
        template_id: row.get(8)?,
        template_version: row.get(9)?,
        summary: row.get(10)?,
        keywords: serde_json::from_str(&keywords_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                11,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        suggested_filename: row.get(12)?,
        category_id: row.get(13)?,
        confidence: row.get(14)?,
        reason: row.get(15)?,
        status: AnalysisResultStatus::parse(&status).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                16,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
            )
        })?,
        created_at: row.get(17)?,
    })
}
