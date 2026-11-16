//! Tanya Data: AI memilih tool -> Rust menjalankan query -> AI merangkum -> backend memverifikasi angka.

use crate::ai::numcheck::{allowed_numbers, unverified_numbers};
use crate::ai::tools::{self, select_prompt, select_schema, summary_prompt, summary_schema, SummaryOut, ToolChoice, ToolContext, Validated};
use crate::ai::{AiPurpose, AiRequest};
use crate::auth::CurrentUser;
use crate::error::{AppError, AppResult};
use crate::filters::{default_range, reference_date};
use crate::settings::Settings;
use crate::state::AppState;
use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Instant;

#[derive(Deserialize)]
pub struct AskReq {
    pub question: String,
}

pub async fn tool_context(st: &AppState) -> AppResult<ToolContext> {
    let reference = reference_date(st).await?;
    let (default_from, default_to) = default_range(reference);
    let settings = Settings::load(&st.db).await?;
    let categories: Vec<String> = sqlx::query_scalar("SELECT name FROM categories ORDER BY name").fetch_all(&st.db).await?;
    let commodities: Vec<String> = sqlx::query_scalar("SELECT name FROM commodities ORDER BY name").fetch_all(&st.db).await?;
    Ok(ToolContext { reference, default_from, default_to, weights: settings.weights(), anomaly: settings.anomaly(), categories, commodities })
}

pub async fn ask(State(st): State<AppState>, user: CurrentUser, Json(req): Json<AskReq>) -> AppResult<Json<Value>> {
    let question = req.question.trim().to_string();
    if question.is_empty() {
        return Err(AppError::BadRequest("Pertanyaan tidak boleh kosong".into()));
    }
    if question.chars().count() > 500 {
        return Err(AppError::BadRequest("Pertanyaan terlalu panjang (maks. 500 karakter)".into()));
    }
    let started = Instant::now();
    let ctx = tool_context(&st).await?;
    let mut attempts: Vec<Value> = Vec::new();

    // 1) AI memilih tool + parameter (maks. 2x perbaikan)
    let mut feedback: Option<String> = None;
    let mut chosen: Option<(ToolChoice, Validated)> = None;
    let mut ai_error: Option<String> = None;
    for _ in 0..3 {
        let req = AiRequest {
            purpose: AiPurpose::SelectTool { question: question.clone(), reference_date: ctx.reference },
            prompt: select_prompt(&question, &ctx, feedback.as_deref()),
            schema: select_schema(),
        };
        match st.ai.call::<ToolChoice>(&req).await {
            Ok(choice) => match tools::validate(&st.db, &ctx, &choice).await? {
                Ok(v) => {
                    attempts.push(json!({ "tool": choice.tool, "params": choice.params, "valid": true }));
                    chosen = Some((choice, v));
                    break;
                }
                Err(e) => {
                    attempts.push(json!({ "tool": choice.tool, "params": choice.params, "valid": false, "error": e }));
                    feedback = Some(e);
                }
            },
            Err(e) => {
                ai_error = Some(e.to_string());
                break;
            }
        }
    }

    let Some((choice, validated)) = chosen else {
        let answer = format!(
            "Maaf, saya belum bisa menjawab pertanyaan itu{}. Coba pertanyaan seperti: \"{}\" atau \"{}\".",
            ai_error.as_ref().map(|e| format!(" ({e})")).unwrap_or_default(),
            tools::EXAMPLE_QUESTIONS[0],
            tools::EXAMPLE_QUESTIONS[1]
        );
        let resp = json!({ "answer": answer, "table": null, "tool": null, "attempts": attempts, "verified": false,
                           "unanswerable": true, "provider": st.ai.provider_name(), "duration_ms": started.elapsed().as_millis(),
                           "suggestions": tools::EXAMPLE_QUESTIONS });
        save_history(&st, user.id, &question, &resp).await?;
        return Ok(Json(resp));
    };

    // 2) Rust menjalankan query terparameter
    let result = tools::execute(&st.db, &ctx, &validated, &choice.tool).await?;

    // 3) AI merangkum; angka diverifikasi terhadap tabel
    let (answer, verified, fallback, fallback_reason) = if matches!(validated, Validated::None) || result.rows.is_empty() {
        (tools::template_summary(&result), true, false, None)
    } else {
        let mut extra = vec![question.clone(), result.title.clone(), result.note.clone().unwrap_or_default(), result.params.to_string()];
        extra.push(result.rows.len().to_string());
        let allowed = allowed_numbers(&[json!(result.rows)], &extra);
        let mut fb: Option<String> = None;
        let mut out: Option<String> = None;
        let mut reason: Option<String> = None;
        for _ in 0..2 {
            let req = AiRequest {
                purpose: AiPurpose::Summarize { result: result.clone() },
                prompt: summary_prompt(&question, &result, fb.as_deref()),
                schema: summary_schema(),
            };
            match st.ai.call::<SummaryOut>(&req).await {
                Ok(s) => {
                    let bad = unverified_numbers(&s.summary, &allowed);
                    if bad.is_empty() {
                        out = Some(s.summary);
                        break;
                    }
                    reason = Some(format!("ringkasan AI memuat angka yang tidak ada di tabel: {}", bad.join(", ")));
                    fb = Some(bad.join(", "));
                }
                Err(e) => {
                    reason = Some(e.to_string());
                    break;
                }
            }
        }
        match out {
            Some(s) => (s, true, false, None),
            None => (tools::template_summary(&result), true, true, reason),
        }
    };

    let resp = json!({
        "answer": answer,
        "verified": verified,
        "fallback": fallback,
        "fallback_reason": fallback_reason,
        "unanswerable": matches!(validated, Validated::None),
        "tool": { "name": choice.tool, "params": result.params, "raw_params": choice.params, "reason": choice.reason },
        "table": if result.columns.is_empty() { Value::Null } else { json!({ "title": result.title, "columns": result.columns, "rows": result.rows, "note": result.note }) },
        "attempts": attempts,
        "provider": st.ai.provider_name(),
        "duration_ms": started.elapsed().as_millis(),
        "suggestions": if matches!(validated, Validated::None) { json!(tools::EXAMPLE_QUESTIONS[..6]) } else { Value::Null },
    });
    save_history(&st, user.id, &question, &resp).await?;
    Ok(Json(resp))
}

async fn save_history(st: &AppState, user_id: i64, question: &str, resp: &Value) -> AppResult<()> {
    let answer = resp.get("answer").and_then(Value::as_str).unwrap_or_default();
    sqlx::query("INSERT INTO ask_history(user_id, question, tool_calls_json, answer) VALUES (?, ?, ?, ?)")
        .bind(user_id)
        .bind(question)
        .bind(resp.to_string())
        .bind(answer)
        .execute(&st.db)
        .await?;
    Ok(())
}

pub async fn history(State(st): State<AppState>, user: CurrentUser) -> AppResult<Json<Value>> {
    let rows: Vec<(i64, String, String, String)> = sqlx::query_as(
        "SELECT id, question, tool_calls_json, created_at FROM ask_history WHERE user_id = ? ORDER BY id DESC LIMIT 50",
    )
    .bind(user.id)
    .fetch_all(&st.db)
    .await?;
    let items: Vec<Value> = rows
        .into_iter()
        .map(|(id, q, resp, at)| json!({ "id": id, "question": q, "response": serde_json::from_str::<Value>(&resp).unwrap_or(Value::Null), "created_at": at }))
        .collect();
    Ok(Json(json!({ "history": items, "examples": tools::EXAMPLE_QUESTIONS })))
}
